use std::io;
use std::path::Path;

use serde_json::{Value, json};

use super::HookKind;

pub struct HookFile {
    pub path: &'static str,
    pub hooks: &'static [(HookKind, &'static str, Option<&'static str>)],
    allow: bool,
    flat: bool,
}

pub const CLAUDE: HookFile = HookFile {
    path: ".claude/settings.json",
    hooks: &[
        (HookKind::Create, "WorktreeCreate", None),
        (HookKind::Remove, "WorktreeRemove", None),
        (HookKind::Stop, "SubagentStop", None),
        (
            HookKind::Guard,
            "PreToolUse",
            Some("Bash|Edit|Write|NotebookEdit"),
        ),
    ],
    allow: true,
    flat: false,
};

pub const SETTINGS: &str = CLAUDE.path;

pub const CODEX: HookFile = HookFile {
    path: ".codex/hooks.json",
    hooks: &[(HookKind::Guard, "PreToolUse", Some("Bash"))],
    allow: false,
    flat: false,
};

pub const GEMINI: HookFile = HookFile {
    path: ".gemini/settings.json",
    hooks: &[(
        HookKind::Guard,
        "BeforeTool",
        Some("run_shell_command|write_file|replace"),
    )],
    allow: false,
    flat: false,
};

pub const COPILOT: HookFile = HookFile {
    path: ".github/hooks/dx.json",
    hooks: &[(HookKind::Guard, "PreToolUse", Some("Bash"))],
    allow: false,
    flat: true,
};

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
        if let Some(matcher) = matcher
            && let Some(object) = entry.as_object_mut()
        {
            object.insert("matcher".to_owned(), json!(matcher));
        }
        return entry;
    }
    let hooks = json!([{ "type": "command", "command": command }]);
    match matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": hooks }),
        None => json!({ "hooks": hooks }),
    }
}

const ALLOW: [&str; 4] = ["check", "test", "build", "status"];

const EXPORT_ALLOW: [&str; 2] = ["DX_WORKTREE_SESSION", "DX_WORKTREE_HOLDER"];

#[must_use]
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

const ASK: [&str; 2] = [
    "Bash(tools/dx deploy --apply *)",
    "Bash(tools/dx deploy * --apply*)",
];

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

#[must_use]
pub fn missing_allow(existing: &str) -> Vec<String> {
    missing_rules(existing, "allow", allow_rules())
}

#[must_use]
pub fn missing_ask(existing: &str) -> Vec<String> {
    missing_rules(existing, "ask", ask_rules())
}

#[must_use]
pub fn settings_text(file: &HookFile) -> String {
    let mut hooks = serde_json::Map::new();
    for (event, matcher, command) in entries(file) {
        hooks.insert(
            event.to_owned(),
            json!([entry_value(file, matcher, &command)]),
        );
    }
    let mut settings = json!({ "hooks": hooks });
    if let Some(object) = settings.as_object_mut() {
        if file.flat {
            object.insert("version".to_owned(), json!(1));
        }
        if file.allow {
            object.insert(
                "permissions".to_owned(),
                json!({ "allow": allow_rules(), "ask": ask_rules() }),
            );
        }
    }
    let text = serde_json::to_string_pretty(&settings).unwrap_or_default();
    format!("{text}\n")
}

fn command_entries<'a>(
    file: &HookFile,
    existing: &'a Value,
    event: &str,
    command: &str,
) -> Vec<&'a Value> {
    let key = if file.flat { "bash" } else { "command" };
    existing
        .get("hooks")
        .and_then(|hooks| hooks.get(event))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|group| {
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
        .collect()
}

fn has_command(
    file: &HookFile,
    existing: &Value,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) -> bool {
    command_entries(file, existing, event, command)
        .into_iter()
        .any(|group| group.get("matcher").and_then(Value::as_str) == matcher)
}

fn stale_entries(
    file: &HookFile,
    existing: &Value,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) -> Vec<Value> {
    command_entries(file, existing, event, command)
        .into_iter()
        .filter(|group| {
            let other = group.get("matcher").and_then(Value::as_str);
            other != matcher && **group == entry_value(file, other, command)
        })
        .cloned()
        .collect()
}

#[must_use]
pub fn missing_entries(file: &HookFile, existing: &str) -> Vec<String> {
    let parsed = parse(existing);
    entries(file)
        .into_iter()
        .filter(|(event, matcher, command)| !has_command(file, &parsed, event, *matcher, command))
        .map(|(event, matcher, command)| match matcher {
            Some(matcher) => format!("{event} ({matcher}): {command}"),
            None => format!("{event}: {command}"),
        })
        .collect()
}

#[must_use]
pub fn workspace_missing(workspace: &Path) -> Vec<(&'static str, Vec<String>)> {
    HOOK_FILES
        .into_iter()
        .filter_map(|file| {
            let Ok(existing) = std::fs::read_to_string(workspace.join(file.path)) else {
                return None;
            };
            let missing = missing_entries(file, &existing);
            (!missing.is_empty()).then_some((file.path, missing))
        })
        .collect()
}

pub struct Outcome {
    pub wrote: Vec<String>,
    pub missing: Vec<(&'static str, Vec<String>)>,
    pub missing_allow: Vec<String>,
    pub missing_ask: Vec<String>,
    pub unparsable: Vec<&'static str>,
}

pub fn apply(workspace: &Path, create: &[&str]) -> io::Result<Outcome> {
    sync(workspace, true, create)
}

pub fn refresh(workspace: &Path) -> io::Result<Outcome> {
    sync(workspace, true, &[])
}

pub fn inspect(workspace: &Path, create: &[&str]) -> io::Result<Outcome> {
    sync(workspace, false, create)
}

fn merge(file: &HookFile, existing: &str) -> Result<String, dx_merge::Error> {
    let parsed = parse(existing);
    let mut text = existing.to_owned();
    for (event, matcher, command) in entries(file) {
        if !has_command(file, &parsed, event, matcher, &command) {
            let stale = stale_entries(file, &parsed, event, matcher, &command);
            if !stale.is_empty() {
                text = dx_merge::jsonc::remove_array_values(&text, &["hooks", event], &stale)?;
            }
            text = dx_merge::jsonc::append_missing(
                &text,
                &["hooks", event],
                &entry_value(file, matcher, &command),
            )?;
        }
    }
    if file.allow {
        for rule in missing_allow(existing) {
            text = dx_merge::jsonc::append_missing(&text, &["permissions", "allow"], &json!(rule))?;
        }
        for rule in missing_ask(existing) {
            text = dx_merge::jsonc::append_missing(&text, &["permissions", "ask"], &json!(rule))?;
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
        unparsable: Vec::new(),
    };
    for file in HOOK_FILES {
        let path = workspace.join(file.path);
        if path.exists() {
            let existing = std::fs::read_to_string(&path)?;
            if !matches!(dx_merge::jsonc::parse(&existing), Ok(Value::Object(_))) {
                outcome.unparsable.push(file.path);
                continue;
            }
            let (rules, ask) = if file.allow {
                (missing_allow(&existing), missing_ask(&existing))
            } else {
                (Vec::new(), Vec::new())
            };
            let missing = missing_entries(file, &existing);
            if write && (!missing.is_empty() || !rules.is_empty() || !ask.is_empty()) {
                match merge(file, &existing) {
                    Ok(merged) => {
                        dx_atomic_fs::write_atomic(&path, merged.as_bytes())?;
                        outcome.wrote.push(file.path.to_owned());
                        continue;
                    }
                    Err(_) => outcome.unparsable.push(file.path),
                }
            }
            outcome.missing_allow.extend(rules);
            outcome.missing_ask.extend(ask);
            if !missing.is_empty() {
                outcome.missing.push((file.path, missing));
            }
        } else if create.contains(&file.path) {
            if write {
                dx_atomic_fs::write_atomic(&path, settings_text(file).as_bytes())?;
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
        let dir = scratch("dx-worktree-setup-refresh-").unwrap();
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
        let dir = scratch("dx-worktree-setup-fresh-").unwrap();
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
        assert_eq!(
            workspace_missing(dir.path()),
            [] as [(&str, std::vec::Vec<std::string::String>); 0]
        );
    }

    #[test]
    fn apply_creates_only_the_paths_it_is_given_and_reports_only_existing_files() {
        let dir = scratch("dx-worktree-setup-create-").unwrap();
        let outcome = apply(dir.path(), &[CODEX.path]).expect("apply");
        assert_eq!(outcome.wrote, vec![CODEX.path.to_owned()]);
        assert!(!dir.path().join(CLAUDE.path).exists());
        assert_eq!(
            workspace_missing(dir.path()),
            [] as [(&str, std::vec::Vec<std::string::String>); 0]
        );
    }

    #[test]
    fn inspect_writes_nothing_and_fails_on_a_directory() {
        let dir = scratch("dx-worktree-setup-inspect-").unwrap();
        let outcome = inspect(dir.path(), &ALL).expect("inspect");
        assert_eq!(outcome.wrote.len(), HOOK_FILES.len());
        assert!(!dir.path().join(".claude").exists());
        std::fs::create_dir_all(dir.path().join(CLAUDE.path)).expect("dir");
        assert!(inspect(dir.path(), &ALL).is_err());
    }

    const USER_SETTINGS: &str = "{\n  // mine\n  \"hooks\": {\n    \"PreToolUse\": [\n      { \"matcher\": \"Bash\", \"hooks\": [{ \"type\": \"command\", \"command\": \"mine\" }] },\n    ],\n  },\n  \"model\": \"opus\", /* keep */\n}\n";

    #[test]
    fn missing_entries_are_added_in_place_and_user_bytes_stay() {
        let dir = scratch("dx-worktree-setup-merge-").unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        std::fs::write(dir.path().join(CLAUDE.path), USER_SETTINGS).expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert!(outcome.wrote.contains(&CLAUDE.path.to_owned()));
        assert!(outcome.missing.is_empty() && outcome.missing_allow.is_empty());
        let text = std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read");
        assert!(text.contains("// mine"));
        assert!(text.contains("/* keep */"));
        assert!(text.contains("\"command\": \"mine\""));
        assert_eq!(missing_entries(&CLAUDE, &text), [] as [String; 0]);
        assert_eq!(missing_allow(&text), [] as [String; 0]);
        assert_eq!(missing_ask(&text), [] as [String; 0]);
        let parsed = parse(&text);
        assert_eq!(commands(&parsed, "PreToolUse").len(), 2);
        assert_eq!(parsed["model"], "opus");
    }

    #[test]
    fn inspect_leaves_an_existing_file_and_reports_what_is_missing() {
        let dir = scratch("dx-worktree-setup-report-").unwrap();
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
        let dir = scratch("dx-worktree-setup-merge-twice-").unwrap();
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
        let dir = scratch("dx-worktree-setup-gemini-").unwrap();
        std::fs::create_dir_all(dir.path().join(".gemini")).expect("dir");
        let own = "{\"general\": {\"vimMode\": true}}\n";
        std::fs::write(dir.path().join(GEMINI.path), own).expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert!(outcome.wrote.contains(&GEMINI.path.to_owned()));
        let text = std::fs::read_to_string(dir.path().join(GEMINI.path)).expect("read");
        assert_eq!(parse(&text)["general"]["vimMode"], true);
        assert_eq!(missing_entries(&GEMINI, &text), [] as [String; 0]);
        assert!(!text.contains("permissions"));
    }

    #[test]
    fn the_copilot_file_is_flat_and_merges_into_a_users_hooks() {
        let dir = scratch("dx-worktree-setup-copilot-").unwrap();
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
        assert_eq!(missing_entries(&COPILOT, &text), [] as [String; 0]);
        assert_eq!(
            parse(&text)["hooks"]["PreToolUse"].as_array().map(Vec::len),
            Some(2)
        );
    }

    #[test]
    fn an_unparsable_file_is_left_alone_and_reported() {
        let dir = scratch("dx-worktree-setup-broken-").unwrap();
        std::fs::create_dir_all(dir.path().join(".gemini")).expect("dir");
        std::fs::write(dir.path().join(GEMINI.path), "{ nope").expect("settings");
        let outcome = apply(dir.path(), &ALL).expect("apply");
        assert_eq!(outcome.unparsable, vec![GEMINI.path]);
        assert!(!outcome.wrote.contains(&GEMINI.path.to_owned()));
        assert_eq!(
            std::fs::read_to_string(dir.path().join(GEMINI.path)).expect("read"),
            "{ nope"
        );
    }

    #[test]
    fn truncated_settings_file_is_reported() {
        let fresh = settings_text(&CLAUDE);
        let half = fresh.get(..40).expect("prefix");
        for broken in ["", half] {
            let dir = scratch("dx-worktree-setup-truncated-").unwrap();
            std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
            std::fs::write(dir.path().join(CLAUDE.path), broken).expect("settings");
            for outcome in [
                refresh(dir.path()).expect("refresh"),
                inspect(dir.path(), &ALL).expect("inspect"),
            ] {
                assert_eq!(outcome.unparsable, vec![CLAUDE.path], "{broken:?}");
                assert!(!outcome.wrote.contains(&CLAUDE.path.to_owned()));
            }
            assert_eq!(
                std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read"),
                broken
            );
        }
    }

    #[test]
    fn a_settings_file_merge_cannot_edit_is_reported() {
        let dir = scratch("dx-worktree-setup-wrong-type-").unwrap();
        std::fs::create_dir_all(dir.path().join(".codex")).expect("dir");
        let own = "{\"hooks\": []}\n";
        std::fs::write(dir.path().join(CODEX.path), own).expect("hooks");
        let outcome = refresh(dir.path()).expect("refresh");
        assert_eq!(outcome.unparsable, vec![CODEX.path]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(CODEX.path)).expect("read"),
            own
        );
    }

    #[cfg(unix)]
    #[test]
    fn create_uses_atomic_write() {
        let dir = scratch("dx-worktree-setup-atomic-").unwrap();
        std::fs::create_dir_all(dir.path().join(".codex")).expect("dir");
        let target = dir.path().join("elsewhere.json");
        std::os::unix::fs::symlink(&target, dir.path().join(CODEX.path)).expect("link");
        let outcome = apply(dir.path(), &[CODEX.path]).expect("apply");
        assert_eq!(outcome.wrote, vec![CODEX.path.to_owned()]);
        assert!(!target.exists());
        let meta = std::fs::symlink_metadata(dir.path().join(CODEX.path)).expect("meta");
        assert!(meta.is_file());
        let text = std::fs::read_to_string(dir.path().join(CODEX.path)).expect("read");
        assert_eq!(text, settings_text(&CODEX));
        let names: Vec<_> = std::fs::read_dir(dir.path().join(".codex"))
            .expect("list")
            .collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn no_file_is_reported_when_absent() {
        let dir = scratch("dx-worktree-setup-absent-").unwrap();
        assert_eq!(
            workspace_missing(dir.path()),
            [] as [(&str, std::vec::Vec<std::string::String>); 0]
        );
    }

    #[test]
    fn a_second_apply_changes_nothing() {
        let dir = scratch("dx-worktree-setup-twice-").unwrap();
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
        let dir = scratch("dx-worktree-setup-allow-").unwrap();
        apply(dir.path(), &ALL).expect("apply");
        let settings = std::fs::read_to_string(dir.path().join(SETTINGS)).expect("settings");
        assert_eq!(missing_allow(&settings), [] as [String; 0]);
        assert_eq!(missing_ask(&settings), [] as [String; 0]);
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
        let dir = scratch("dx-worktree-setup-ask-").unwrap();
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
        assert_eq!(missing_allow(&text), [] as [String; 0]);
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
                    HookKind::Bazel | HookKind::Git => continue,
                };
                assert_eq!(commands(&settings, event), vec![expected.to_owned()]);
            }
        }
    }

    #[test]
    fn the_claude_file_carries_every_hook_at_its_claude_event() {
        let kinds: Vec<HookKind> = CLAUDE.hooks.iter().map(|(kind, _, _)| *kind).collect();
        let agent_kinds: Vec<HookKind> = HookKind::ALL
            .into_iter()
            .filter(|kind| kind.event().is_some())
            .collect();
        assert_eq!(kinds, agent_kinds);
        for (kind, event, _) in CLAUDE.hooks {
            assert_eq!(kind.event(), Some(*event));
        }
    }

    #[test]
    fn a_guard_with_an_old_matcher_is_replaced_and_a_users_entry_stays() {
        let dir = scratch("dx-worktree-setup-matcher-").unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        let command = hook_command(HookKind::Guard);
        let old = entry_value(&CLAUDE, Some("Bash"), &command);
        let mine =
            json!({ "matcher": "Bash", "hooks": [{ "type": "command", "command": "mine" }] });
        let own = json!({ "hooks": { "PreToolUse": [old, mine] } }).to_string();
        std::fs::write(dir.path().join(CLAUDE.path), &own).expect("settings");
        assert!(!missing_entries(&CLAUDE, &own).is_empty());
        apply(dir.path(), &ALL).expect("apply");
        let text = std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read");
        assert_eq!(missing_entries(&CLAUDE, &text), [] as [String; 0]);
        let matchers: Vec<Value> = parse(&text)["hooks"]["PreToolUse"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|entry| entry["matcher"].clone())
            .collect();
        assert_eq!(
            matchers,
            [json!("Bash"), json!("Bash|Edit|Write|NotebookEdit")]
        );
        assert!(text.contains("\"mine\""));
    }

    #[test]
    fn hook_commands_with_global_flags_count_as_missing() {
        let text = settings_text(&CLAUDE);
        let flagged = text.replace("/tools/dx worktree", "/tools/dx --verbose worktree");
        assert_ne!(flagged, text);
        assert_eq!(missing_entries(&CLAUDE, &flagged).len(), CLAUDE.hooks.len());
    }
}
