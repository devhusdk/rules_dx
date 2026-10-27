use std::io;
use std::path::Path;

use serde_json::{json, Value};

use super::HookKind;
use crate::args::Command;

/// One agent hooks file and the hooks dx puts in it.
pub(crate) struct HookFile {
    /// The path under the workspace root.
    pub(crate) path: &'static str,
    /// The event and tool matcher that run each hook kind this file carries.
    hooks: &'static [(HookKind, &'static str, Option<&'static str>)],
    /// Whether `dx status` reports the hooks when the file is absent.
    required: bool,
    /// Whether the file carries the Claude Code `permissions.allow` rules.
    allow: bool,
}

/// Claude Code reads this file, and Cursor imports it by default.
pub(crate) const CLAUDE: HookFile = HookFile {
    path: ".claude/settings.json",
    hooks: &[
        (HookKind::Create, "WorktreeCreate", None),
        (HookKind::Remove, "WorktreeRemove", None),
        (HookKind::Stop, "SubagentStop", None),
        (HookKind::Guard, "PreToolUse", Some("Bash")),
    ],
    required: true,
    allow: true,
};

/// The Claude Code settings file.
pub(crate) const SETTINGS: &str = CLAUDE.path;

/// Codex reads this file once the user trusts it with `/hooks`.
pub(crate) const CODEX: HookFile = HookFile {
    path: ".codex/hooks.json",
    hooks: &[(HookKind::Guard, "PreToolUse", Some("Bash"))],
    required: false,
    allow: false,
};

/// Gemini CLI reads the hooks in this file in trusted folders.
pub(crate) const GEMINI: HookFile = HookFile {
    path: ".gemini/settings.json",
    hooks: &[(HookKind::Guard, "BeforeTool", Some("run_shell_command"))],
    required: false,
    allow: false,
};

/// Every agent hooks file dx writes.
pub(crate) const HOOK_FILES: [&HookFile; 3] = [&CLAUDE, &CODEX, &GEMINI];

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

fn entry_value(matcher: Option<&str>, command: &str) -> Value {
    let hooks = json!([{ "type": "command", "command": command }]);
    match matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": hooks }),
        None => json!({ "hooks": hooks }),
    }
}

/// Read-only commands Claude Code may run without asking. Never add a writer.
const ALLOW: [Command; 6] = [
    Command::Check,
    Command::Test,
    Command::Build,
    Command::ListTests,
    Command::Status,
    Command::Version,
];

/// The `permissions.allow` rules a fresh settings file carries.
pub(crate) fn allow_rules() -> Vec<String> {
    ALLOW
        .into_iter()
        .map(|command| format!("Bash(tools/dx {} *)", command.name()))
        .collect()
}

/// Allow rules an existing settings file lacks; the `:*` spelling counts as present.
pub(crate) fn missing_allow(existing: &str) -> Vec<String> {
    let parsed: Value = serde_json::from_str(existing).unwrap_or(Value::Null);
    let have: Vec<&str> = parsed
        .get("permissions")
        .and_then(|permissions| permissions.get("allow"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    allow_rules()
        .into_iter()
        .filter(|rule| {
            let legacy = rule.replace(" *)", ":*)");
            !have.iter().any(|h| *h == rule || *h == legacy)
        })
        .collect()
}

/// The hooks file a fresh repository gets.
pub(crate) fn settings_text(file: &HookFile) -> String {
    let mut hooks = serde_json::Map::new();
    for (event, matcher, command) in entries(file) {
        hooks.insert(event.to_owned(), json!([entry_value(matcher, &command)]));
    }
    let mut settings = json!({ "hooks": hooks });
    if file.allow {
        settings["permissions"] = json!({ "allow": allow_rules() });
    }
    let text = serde_json::to_string_pretty(&settings).unwrap_or_default();
    format!("{text}\n")
}

fn has_command(existing: &Value, event: &str, command: &str) -> bool {
    existing
        .get("hooks")
        .and_then(|hooks| hooks.get(event))
        .and_then(Value::as_array)
        .is_some_and(|groups| {
            groups.iter().any(|group| {
                group
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|list| {
                        list.iter().any(|hook| {
                            hook.get("command").and_then(Value::as_str) == Some(command)
                        })
                    })
            })
        })
}

/// Hook entries an existing hooks file lacks, as `event: command` lines.
pub(crate) fn missing_entries(file: &HookFile, existing: &str) -> Vec<String> {
    let parsed: Value = serde_json::from_str(existing).unwrap_or(Value::Null);
    entries(file)
        .into_iter()
        .filter(|(event, _, command)| !has_command(&parsed, event, command))
        .map(|(event, matcher, command)| match matcher {
            Some(matcher) => format!("{event} ({matcher}): {command}"),
            None => format!("{event}: {command}"),
        })
        .collect()
}

/// The workspace hooks files that lack entries, with the entries each lacks.
///
/// An absent file counts only when it is required, so repositories adopted
/// before dx wrote a file get no new warning.
pub(crate) fn workspace_missing(workspace: &Path) -> Vec<(&'static str, Vec<String>)> {
    HOOK_FILES
        .into_iter()
        .filter_map(|file| {
            let existing = match std::fs::read_to_string(workspace.join(file.path)) {
                Ok(text) => text,
                Err(_) if !file.required => return None,
                Err(_) => String::new(),
            };
            let missing = missing_entries(file, &existing);
            (!missing.is_empty()).then_some((file.path, missing))
        })
        .collect()
}

/// What setup did for the worktree pool, as report data.
pub(crate) struct Outcome {
    pub(crate) wrote: Vec<String>,
    /// Each existing hooks file that lacks entries, with the entries it lacks.
    pub(crate) missing: Vec<(&'static str, Vec<String>)>,
    /// The allow rules an existing Claude Code settings file lacks.
    pub(crate) missing_allow: Vec<String>,
}

/// Writes each hooks file that is absent and never overwrites one.
pub(crate) fn apply(workspace: &Path) -> io::Result<Outcome> {
    let mut outcome = Outcome {
        wrote: Vec::new(),
        missing: Vec::new(),
        missing_allow: Vec::new(),
    };
    for file in HOOK_FILES {
        let path = workspace.join(file.path);
        if path.exists() {
            let existing = std::fs::read_to_string(&path)?;
            if file.allow {
                outcome.missing_allow = missing_allow(&existing);
            }
            let missing = missing_entries(file, &existing);
            if !missing.is_empty() {
                outcome.missing.push((file.path, missing));
            }
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, settings_text(file))?;
            outcome.wrote.push(file.path.to_owned());
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    use dx_test_scratch::scratch;

    #[test]
    fn a_fresh_repository_gets_every_hooks_file() {
        let dir = scratch("dx-worktree-setup-fresh-");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(
            outcome.wrote,
            vec![
                ".claude/settings.json",
                ".codex/hooks.json",
                ".gemini/settings.json"
            ]
        );
        for file in HOOK_FILES {
            let text = std::fs::read_to_string(dir.path().join(file.path)).expect("file");
            assert!(missing_entries(file, &text).is_empty(), "{}", file.path);
        }
        assert!(workspace_missing(dir.path()).is_empty());
    }

    #[test]
    fn existing_files_are_never_overwritten() {
        let dir = scratch("dx-worktree-setup-existing-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        let own = "{\"hooks\": {}}\n";
        std::fs::write(dir.path().join(CLAUDE.path), own).expect("settings");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(CLAUDE.path)).expect("read"),
            own
        );
        let (path, missing) = &outcome.missing[0];
        assert_eq!(*path, CLAUDE.path);
        assert!(missing.iter().any(|entry| entry.contains("WorktreeCreate")));
        assert!(missing
            .iter()
            .any(|entry| entry.contains("PreToolUse (Bash)")));
    }

    #[test]
    fn an_existing_gemini_settings_file_is_kept_and_its_missing_guard_reported() {
        let dir = scratch("dx-worktree-setup-gemini-");
        std::fs::create_dir_all(dir.path().join(".gemini")).expect("dir");
        let own = "{\"general\": {\"vimMode\": true}}\n";
        std::fs::write(dir.path().join(GEMINI.path), own).expect("settings");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(GEMINI.path)).expect("read"),
            own
        );
        assert!(!outcome.wrote.contains(&GEMINI.path.to_owned()));
        assert_eq!(
            outcome.missing,
            vec![(
                GEMINI.path,
                vec![
                    "BeforeTool (run_shell_command): \"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook guard"
                        .to_owned()
                ]
            )]
        );
        assert_eq!(workspace_missing(dir.path()), outcome.missing);
    }

    #[test]
    fn only_the_claude_file_is_reported_when_absent() {
        let dir = scratch("dx-worktree-setup-absent-");
        let missing = workspace_missing(dir.path());
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].0, CLAUDE.path);
        assert_eq!(missing[0].1.len(), HookKind::ALL.len());
    }

    #[test]
    fn a_second_apply_changes_nothing() {
        let dir = scratch("dx-worktree-setup-twice-");
        apply(dir.path()).expect("first");
        let outcome = apply(dir.path()).expect("second");
        assert!(
            outcome.wrote.is_empty()
                && outcome.missing.is_empty()
                && outcome.missing_allow.is_empty()
        );
    }

    #[test]
    fn a_fresh_repository_gets_every_allow_rule() {
        let dir = scratch("dx-worktree-setup-allow-");
        apply(dir.path()).expect("apply");
        let settings = std::fs::read_to_string(dir.path().join(SETTINGS)).expect("settings");
        assert!(missing_allow(&settings).is_empty());
        assert_eq!(allow_rules().len(), 6);
        for file in [&CODEX, &GEMINI] {
            let text = std::fs::read_to_string(dir.path().join(file.path)).expect("file");
            assert!(!text.contains("permissions"), "{}", file.path);
        }
    }

    #[test]
    fn allowlisted_commands_are_read_only() {
        for rule in allow_rules() {
            let name = rule
                .strip_prefix("Bash(tools/dx ")
                .and_then(|r| r.strip_suffix(" *)"))
                .expect("rule shape");
            let command = Command::parse(name).expect("known command");
            assert!(ALLOW.contains(&command));
            assert!(!command.supports_apply(), "{name} takes --apply");
            assert!(crate::args::parse(&[name]).is_ok(), "{name} must parse");
        }
    }

    #[test]
    fn a_hooks_only_file_lacks_every_allow_rule() {
        let hooks_only = "{\"hooks\": {}}\n";
        assert_eq!(missing_allow(hooks_only), allow_rules());
        assert_eq!(missing_allow("not json"), allow_rules());
    }

    #[test]
    fn the_colon_star_form_counts_as_present() {
        let legacy = serde_json::to_string(&json!({ "permissions": { "allow": allow_rules()
            .iter().map(|r| r.replace(" *)", ":*)")).collect::<Vec<_>>() } }))
        .unwrap();
        assert!(missing_allow(&legacy).is_empty());
    }

    fn commands(settings: &Value, event: &str) -> Vec<String> {
        settings["hooks"][event]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|group| group["hooks"].as_array().into_iter().flatten())
            .filter_map(|hook| hook["command"].as_str().map(str::to_owned))
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
