use std::io;
use std::path::Path;

use serde_json::{Value, json};

use super::HookKind;

/// One agent hooks file and the hooks dx puts in it.
pub struct HookFile {
    /// The path under the workspace root.
    pub path: &'static str,
    /// The event and tool matcher that run each hook kind this file carries.
    pub hooks: &'static [(HookKind, &'static str, Option<&'static str>)],
    /// Whether the file carries the Claude Code `permissions.allow` rules.
    allow: bool,
    /// Whether an entry is the hook itself (`bash` key) instead of a group holding a `hooks` list.
    flat: bool,
}

/// Claude Code reads this file, and Cursor imports it by default.
pub const CLAUDE: HookFile = HookFile {
    path: ".claude/settings.json",
    hooks: &[
        (HookKind::Create, "WorktreeCreate", None),
        (HookKind::Remove, "WorktreeRemove", None),
        (HookKind::Stop, "SubagentStop", None),
        (HookKind::Guard, "PreToolUse", Some("Bash")),
    ],
    allow: true,
    flat: false,
};

/// The Claude Code settings file.
pub const SETTINGS: &str = CLAUDE.path;

/// Codex reads this file once the user trusts it with `/hooks`.
pub const CODEX: HookFile = HookFile {
    path: ".codex/hooks.json",
    hooks: &[(HookKind::Guard, "PreToolUse", Some("Bash"))],
    allow: false,
    flat: false,
};

/// Gemini CLI reads the hooks in this file in trusted folders.
pub const GEMINI: HookFile = HookFile {
    path: ".gemini/settings.json",
    hooks: &[(HookKind::Guard, "BeforeTool", Some("run_shell_command"))],
    allow: false,
    flat: false,
};

/// Copilot reads every `.github/hooks/*.json` file. The `PreToolUse` spelling selects the
/// Claude Code payload, so the guard reads it as it reads Claude's.
pub const COPILOT: HookFile = HookFile {
    path: ".github/hooks/dx.json",
    hooks: &[(HookKind::Guard, "PreToolUse", Some("Bash"))],
    allow: false,
    flat: true,
};

/// Every agent hooks file dx writes.
pub const HOOK_FILES: [&HookFile; 4] = [&CLAUDE, &CODEX, &GEMINI, &COPILOT];

fn hook_command(kind: HookKind) -> String {
    format!(
        "\"${{CLAUDE_PROJECT_DIR:-.}}\"/tools/dx worktree hook {}",
        kind.word()
    )
}

fn entries(file: &HookFile) -> Vec<(&'static str, Option<&'static str>, String)> {
    file.hooks
        .iter()
        .map(|(kind, event, matcher)| (*event, *matcher, hook_command(*kind)))
        .collect()
}

fn entry_value(file: &HookFile, matcher: Option<&str>, command: &str) -> Value {
    if file.flat {
        let mut entry = json!({ "type": "command", "bash": command });
        if let Some(matcher) = matcher {
            entry["matcher"] = json!(matcher);
        }
        return entry;
    }
    let hooks = json!([{ "type": "command", "command": command }]);
    match matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": hooks }),
        None => json!({ "hooks": hooks }),
    }
}

/// Read-only commands Claude Code may run without asking. Never add a writer.
const ALLOW: [&str; 4] = ["check", "test", "build", "status"];

/// Variables the guard hook exports ahead of a dx command; each export is its own
/// segment, so Manual mode would prompt for it without a rule.
const EXPORT_ALLOW: [&str; 2] = ["DX_WORKTREE_SESSION", "DX_WORKTREE_HOLDER"];

/// The `permissions.allow` rules a fresh settings file carries.
pub fn allow_rules() -> Vec<String> {
    ALLOW
        .into_iter()
        .map(|command| format!("Bash(tools/dx {command} *)"))
        .chain(
            EXPORT_ALLOW
                .into_iter()
                .map(|var| format!("Bash(export {var}=*)")),
        )
        .collect()
}

/// Commands Claude Code must ask before running: `dx deploy --apply` publishes.
const ASK: [&str; 2] = [
    "Bash(tools/dx deploy --apply *)",
    "Bash(tools/dx deploy * --apply*)",
];

/// The `permissions.ask` rules a fresh settings file carries.
pub fn ask_rules() -> Vec<String> {
    ASK.into_iter().map(str::to_owned).collect()
}

fn parse(text: &str) -> Value {
    dx_merge::jsonc::parse(text).unwrap_or(Value::Null)
}

fn missing_rules(existing: &str, list: &str, rules: Vec<String>) -> Vec<String> {
    let parsed = parse(existing);
    let have: Vec<&str> = parsed
        .get("permissions")
        .and_then(|permissions| permissions.get(list))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    rules
        .into_iter()
        .filter(|rule| !have.contains(&rule.as_str()))
        .collect()
}

/// Allow rules an existing settings file lacks.
pub fn missing_allow(existing: &str) -> Vec<String> {
    missing_rules(existing, "allow", allow_rules())
}

/// Ask rules an existing settings file lacks.
pub fn missing_ask(existing: &str) -> Vec<String> {
    missing_rules(existing, "ask", ask_rules())
}

/// The hooks file a fresh repository gets.
pub fn settings_text(file: &HookFile) -> String {
    let mut hooks = serde_json::Map::new();
    for (event, matcher, command) in entries(file) {
        hooks.insert(
            event.to_owned(),
            json!([entry_value(file, matcher, &command)]),
        );
    }
    let mut settings = json!({ "hooks": hooks });
    if file.flat {
        settings["version"] = json!(1);
    }
    if file.allow {
        settings["permissions"] = json!({ "allow": allow_rules(), "ask": ask_rules() });
    }
    let text = serde_json::to_string_pretty(&settings).unwrap_or_default();
    format!("{text}\n")
}

fn has_command(file: &HookFile, existing: &Value, event: &str, command: &str) -> bool {
    let key = if file.flat { "bash" } else { "command" };
    existing
        .get("hooks")
        .and_then(|hooks| hooks.get(event))
        .and_then(Value::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                if file.flat {
                    return group.get(key).and_then(Value::as_str) == Some(command);
                }
                group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|list| {
                        list.iter()
                            .any(|hook| hook.get(key).and_then(Value::as_str) == Some(command))
                    })
            })
        })
}

/// Hook entries an existing hooks file lacks, as `event: command` lines.
pub fn missing_entries(file: &HookFile, existing: &str) -> Vec<String> {
    let parsed = parse(existing);
    entries(file)
        .into_iter()
        .filter(|(event, _, command)| !has_command(file, &parsed, event, command))
        .map(|(event, matcher, command)| match matcher {
            Some(matcher) => format!("{event} ({matcher}): {command}"),
            None => format!("{event}: {command}"),
        })
        .collect()
}

/// The existing workspace hooks files that lack entries, with the entries each lacks.
///
/// An absent file is never reported; the agent it belongs to was not chosen.
pub fn workspace_missing(workspace: &Path) -> Vec<(&'static str, Vec<String>)> {
    HOOK_FILES
        .into_iter()
        .filter_map(|file| {
            let existing = match std::fs::read_to_string(workspace.join(file.path)) {
                Ok(text) => text,
                Err(_) => return None,
            };
            let missing = missing_entries(file, &existing);
            (!missing.is_empty()).then_some((file.path, missing))
        })
        .collect()
}

/// What setup did for the worktree pool, as report data.
pub struct Outcome {
    pub wrote: Vec<String>,
    /// Each existing hooks file that lacks entries, with the entries it lacks.
    pub missing: Vec<(&'static str, Vec<String>)>,
    /// The allow rules an existing Claude Code settings file lacks.
    pub missing_allow: Vec<String>,
    /// The ask rules an existing Claude Code settings file lacks.
    pub missing_ask: Vec<String>,
}

/// Writes each absent hooks file whose path is in `create` and adds missing entries to existing ones in place.
pub fn apply(workspace: &Path, create: &[&str]) -> io::Result<Outcome> {
    sync(workspace, true, create)
}

/// Merges missing entries into existing hooks files; absent files stay absent.
pub fn refresh(workspace: &Path) -> io::Result<Outcome> {
    sync(workspace, true, &[])
}

/// What [`apply`] would do, without writing; fails where it would fail to read.
pub fn inspect(workspace: &Path, create: &[&str]) -> io::Result<Outcome> {
    sync(workspace, false, create)
}

fn merge(file: &HookFile, existing: &str) -> Result<String, dx_merge::Error> {
    let parsed = parse(existing);
    let mut text = existing.to_owned();
    for (event, matcher, command) in entries(file) {
        if !has_command(file, &parsed, event, &command) {
            text = dx_merge::jsonc::append_missing(
                &text,
                &["hooks", event],
                entry_value(file, matcher, &command),
            )?;
        }
    }
    if file.allow {
        for rule in missing_allow(existing) {
            text = dx_merge::jsonc::append_missing(&text, &["permissions", "allow"], json!(rule))?;
        }
        for rule in missing_ask(existing) {
            text = dx_merge::jsonc::append_missing(&text, &["permissions", "ask"], json!(rule))?;
        }
    }
    Ok(text)
}

fn sync(workspace: &Path, write: bool, create: &[&str]) -> io::Result<Outcome> {
    let mut outcome = Outcome {
        wrote: Vec::new(),
        missing: Vec::new(),
        missing_allow: Vec::new(),
        missing_ask: Vec::new(),
    };
    for file in HOOK_FILES {
        let path = workspace.join(file.path);
        if path.exists() {
            let existing = std::fs::read_to_string(&path)?;
            let (rules, ask) = if file.allow {
                (missing_allow(&existing), missing_ask(&existing))
            } else {
                (Vec::new(), Vec::new())
            };
            let missing = missing_entries(file, &existing);
            if write
                && (!missing.is_empty() || !rules.is_empty() || !ask.is_empty())
                && let Ok(merged) = merge(file, &existing)
            {
                dx_atomic_fs::write_atomic(&path, merged.as_bytes())?;
                outcome.wrote.push(file.path.to_owned());
                continue;
            }
            outcome.missing_allow.extend(rules);
            outcome.missing_ask.extend(ask);
            if !missing.is_empty() {
                outcome.missing.push((file.path, missing));
            }
        } else if create.contains(&file.path) {
            if write {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, settings_text(file))?;
            }
            outcome.wrote.push(file.path.to_owned());
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_creates_nothing_and_merges_into_an_existing_file() {
        let dir = scratch("dx-worktree-setup-refresh-");
        let path = HOOK_FILES[0].path;
        std::fs::create_dir_all(dir.path().join(path).parent().expect("parent")).expect("dir");
        std::fs::write(dir.path().join(path), "{}").expect("write");
        let outcome = refresh(dir.path()).expect("refresh");
        assert_eq!(outcome.wrote, vec![path.to_owned()]);
        for file in &HOOK_FILES[1..] {
            assert!(!dir.path().join(file.path).exists(), "{}", file.path);
        }
    }

    use dx_test_scratch::scratch;

    const ALL: [&str; 4] = [
        ".claude/settings.json",
        ".codex/hooks.json",
        ".gemini/settings.json",
        ".github/hooks/dx.json",
    ];

    #[test]
    fn a_fresh_repository_gets_every_hooks_file() {
        let dir = scratch("dx-worktree-setup-fresh-");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert_eq!(
            outcome.wrote,
            vec![
                ".claude/settings.json",
                ".codex/hooks.json",
                ".gemini/settings.json",
                ".github/hooks/dx.json"
            ]
        );
        for file in HOOK_FILES {
            let text = std::fs::read_to_string(dir.path().join(file.path)).expect("file");
            assert!(missing_entries(file, &text).is_empty(), "{}", file.path);
        }
        assert!(workspace_missing(dir.path()).is_empty());
    }

    #[test]
    fn apply_creates_only_the_paths_it_is_given_and_reports_only_existing_files() {
        let dir = scratch("dx-worktree-setup-create-");
        let outcome = apply(dir.path(), &[CODEX.path]).expect("apply");
        assert_eq!(outcome.wrote, vec![CODEX.path.to_owned()]);
        assert!(!dir.path().join(CLAUDE.path).exists());
        assert!(workspace_missing(dir.path()).is_empty());
    }

    #[test]
    fn inspect_writes_nothing_and_fails_on_a_directory() {
        let dir = scratch("dx-worktree-setup-inspect-");
        let outcome = inspect(dir.path(), &ALL).expect("inspect");
        assert_eq!(outcome.wrote.len(), HOOK_FILES.len());
        assert!(!dir.path().join(".claude").exists());
        std::fs::create_dir_all(dir.path().join(CLAUDE.path)).expect("dir");
        assert!(inspect(dir.path(), &ALL).is_err());
    }

    const USER_SETTINGS: &str = "{\n  // mine\n  \"hooks\": {\n    \"PreToolUse\": [\n      { \"matcher\": \"Bash\", \"hooks\": [{ \"type\": \"command\", \"command\": \"mine\" }] },\n    ],\n  },\n  \"model\": \"opus\", /* keep */\n}\n";

    #[test]
    fn missing_entries_are_added_in_place_and_user_bytes_stay() {
        let dir = scratch("dx-worktree-setup-merge-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        std::fs::write(dir.path().join(CLAUDE.path), USER_SETTINGS).expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert!(outcome.wrote.contains(&CLAUDE.path.to_owned()));
        assert!(outcome.missing.is_empty() && outcome.missing_allow.is_empty());
        let text = std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read");
        assert!(text.contains("// mine"));
        assert!(text.contains("/* keep */"));
        assert!(text.contains("\"command\": \"mine\""));
        assert!(missing_entries(&CLAUDE, &text).is_empty());
        assert!(missing_allow(&text).is_empty());
        assert!(missing_ask(&text).is_empty());
        let parsed = parse(&text);
        assert_eq!(commands(&parsed, "PreToolUse").len(), 2);
        assert_eq!(parsed["model"], "opus");
    }

    #[test]
    fn inspect_leaves_an_existing_file_and_reports_what_is_missing() {
        let dir = scratch("dx-worktree-setup-report-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        std::fs::write(dir.path().join(CLAUDE.path), USER_SETTINGS).expect("settings");
        let outcome = inspect(dir.path(), &ALL).expect("inspect");
        let (path, missing) = &outcome.missing[0];
        assert_eq!(*path, CLAUDE.path);
        assert!(missing.iter().any(|entry| entry.contains("WorktreeCreate")));
        assert_eq!(outcome.missing_allow, allow_rules());
        assert_eq!(outcome.missing_ask, ask_rules());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read"),
            USER_SETTINGS
        );
    }

    #[test]
    fn a_second_apply_writes_zero_bytes_to_a_merged_file() {
        let dir = scratch("dx-worktree-setup-merge-twice-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        std::fs::write(dir.path().join(CLAUDE.path), USER_SETTINGS).expect("settings");
        apply(dir.path(), &ALL).expect("first");
        let once = std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read");
        let outcome = apply(dir.path(), &ALL).expect("second");
        assert!(!outcome.wrote.contains(&CLAUDE.path.to_owned()));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read"),
            once
        );
    }

    #[test]
    fn an_existing_gemini_settings_file_gains_only_its_guard() {
        let dir = scratch("dx-worktree-setup-gemini-");
        std::fs::create_dir_all(dir.path().join(".gemini")).expect("dir");
        let own = "{\"general\": {\"vimMode\": true}}\n";
        std::fs::write(dir.path().join(GEMINI.path), own).expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert!(outcome.wrote.contains(&GEMINI.path.to_owned()));
        let text = std::fs::read_to_string(dir.path().join(GEMINI.path)).expect("read");
        assert_eq!(parse(&text)["general"]["vimMode"], true);
        assert!(missing_entries(&GEMINI, &text).is_empty());
        assert!(!text.contains("permissions"));
    }

    #[test]
    fn the_copilot_file_is_flat_and_merges_into_a_users_hooks() {
        let dir = scratch("dx-worktree-setup-copilot-");
        let fresh = settings_text(&COPILOT);
        let parsed = parse(&fresh);
        assert_eq!(parsed["version"], 1);
        let entry = &parsed["hooks"]["PreToolUse"][0];
        assert_eq!(entry["matcher"], "Bash");
        assert!(
            entry["bash"]
                .as_str()
                .expect("bash")
                .ends_with("worktree hook guard")
        );
        std::fs::create_dir_all(dir.path().join(".github/hooks")).expect("dir");
        let own = "{\"version\": 1, \"hooks\": {\"PreToolUse\": [{\"bash\": \"mine\"}]}}\n";
        std::fs::write(dir.path().join(COPILOT.path), own).expect("hooks");
        apply(dir.path(), &ALL).expect("apply");
        let text = std::fs::read_to_string(dir.path().join(COPILOT.path)).expect("read");
        assert!(text.contains("\"mine\""));
        assert!(missing_entries(&COPILOT, &text).is_empty());
        assert_eq!(
            parse(&text)["hooks"]["PreToolUse"].as_array().map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn an_unparsable_file_is_left_alone_and_reported() {
        let dir = scratch("dx-worktree-setup-broken-");
        std::fs::create_dir_all(dir.path().join(".gemini")).expect("dir");
        std::fs::write(dir.path().join(GEMINI.path), "{ nope").expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert_eq!(outcome.missing[0].0, GEMINI.path);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(GEMINI.path)).expect("read"),
            "{ nope"
        );
    }

    #[test]
    fn no_file_is_reported_when_absent() {
        let dir = scratch("dx-worktree-setup-absent-");
        assert!(workspace_missing(dir.path()).is_empty());
    }

    #[test]
    fn a_second_apply_changes_nothing() {
        let dir = scratch("dx-worktree-setup-twice-");
        apply(dir.path(), &ALL).expect("first");
        let outcome = apply(dir.path(), &ALL).expect("second");
        assert!(
            outcome.wrote.is_empty()
                && outcome.missing.is_empty()
                && outcome.missing_allow.is_empty()
        );
    }

    #[test]
    fn a_fresh_repository_gets_every_allow_rule() {
        let dir = scratch("dx-worktree-setup-allow-");
        apply(dir.path(), &ALL).expect("apply");
        let settings = std::fs::read_to_string(dir.path().join(SETTINGS)).expect("settings");
        assert!(missing_allow(&settings).is_empty());
        assert!(missing_ask(&settings).is_empty());
        assert_eq!(allow_rules().len(), 6);
        assert!(!allow_rules().iter().any(|rule| rule.contains("deploy")));
        assert!(
            ask_rules()
                .iter()
                .all(|rule| rule.contains("tools/dx deploy"))
        );
        for file in [&CODEX, &GEMINI] {
            let text = std::fs::read_to_string(dir.path().join(file.path)).expect("file");
            assert!(!text.contains("permissions"), "{}", file.path);
        }
    }

    #[test]
    fn a_hooks_only_file_lacks_every_allow_rule() {
        let hooks_only = "{\"hooks\": {}}\n";
        assert_eq!(missing_allow(hooks_only), allow_rules());
        assert_eq!(missing_allow("not json"), allow_rules());
        assert_eq!(missing_ask(hooks_only), ask_rules());
    }

    #[test]
    fn an_existing_allow_list_gains_the_deploy_ask_rules() {
        let dir = scratch("dx-worktree-setup-ask-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        let fresh = settings_text(&CLAUDE);
        let mut parsed = parse(&fresh);
        parsed["permissions"]
            .as_object_mut()
            .expect("permissions")
            .remove("ask");
        let old = serde_json::to_string_pretty(&parsed).expect("json");
        std::fs::write(dir.path().join(CLAUDE.path), &old).expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert!(outcome.wrote.contains(&CLAUDE.path.to_owned()));
        let text = std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read");
        assert_eq!(parse(&text)["permissions"]["ask"], json!(ask_rules()));
        assert!(missing_allow(&text).is_empty());
    }

    fn commands(settings: &Value, event: &str) -> Vec<String> {
        settings["hooks"][event]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|group| match group["hooks"].as_array() {
                Some(list) => list.clone(),
                None => vec![group.clone()],
            })
            .filter_map(|hook| {
                hook["command"]
                    .as_str()
                    .or(hook["bash"].as_str())
                    .map(str::to_owned)
            })
            .collect()
    }

    #[test]
    fn every_generated_hook_command_is_the_bare_fast_path() {
        for file in HOOK_FILES {
            let settings: Value =
                serde_json::from_str(&settings_text(file)).expect("settings json");
            let events = settings["hooks"].as_object().expect("hooks object");
            let total: usize = events
                .keys()
                .map(|event| commands(&settings, event).len())
                .sum();
            assert_eq!(total, file.hooks.len(), "{}", file.path);
            for (kind, event, _) in file.hooks {
                let expected = match kind {
                    HookKind::Create => {
                        "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook create"
                    }
                    HookKind::Remove => {
                        "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook remove"
                    }
                    HookKind::Stop => "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook stop",
                    HookKind::Guard => "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook guard",
                };
                assert_eq!(commands(&settings, event), vec![expected.to_owned()]);
            }
        }
    }

    #[test]
    fn the_claude_file_carries_every_hook_at_its_claude_event() {
        let kinds: Vec<HookKind> = CLAUDE.hooks.iter().map(|(kind, _, _)| *kind).collect();
        assert_eq!(kinds, HookKind::ALL);
        for (kind, event, _) in CLAUDE.hooks {
            assert_eq!(kind.event(), *event);
        }
    }

    #[test]
    fn hook_commands_with_global_flags_count_as_missing() {
        let text = settings_text(&CLAUDE);
        let flagged = text.replace("/tools/dx worktree", "/tools/dx --verbose worktree");
        assert_ne!(flagged, text);
        assert_eq!(
            missing_entries(&CLAUDE, &flagged).len(),
            HookKind::ALL.len()
        );
    }
}
