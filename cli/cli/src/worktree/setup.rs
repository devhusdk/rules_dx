use std::io;
use std::path::Path;

use serde_json::{json, Value};

use super::HookKind;
use crate::args::Command;

pub(crate) const SETTINGS: &str = ".claude/settings.json";

fn hook_command(kind: HookKind) -> String {
    format!(
        "\"${{CLAUDE_PROJECT_DIR:-.}}\"/tools/dx worktree hook {}",
        kind.word()
    )
}

fn hook_event(kind: HookKind) -> (&'static str, Option<&'static str>) {
    let matcher = (kind == HookKind::Guard).then_some("Bash");
    (kind.event(), matcher)
}

fn entries() -> Vec<(&'static str, Option<&'static str>, String)> {
    HookKind::ALL
        .into_iter()
        .map(|kind| {
            let (name, matcher) = hook_event(kind);
            (name, matcher, hook_command(kind))
        })
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

/// The settings file a fresh repository gets.
pub(crate) fn settings_text() -> String {
    let mut hooks = serde_json::Map::new();
    for (event, matcher, command) in entries() {
        hooks.insert(event.to_owned(), json!([entry_value(matcher, &command)]));
    }
    let text = serde_json::to_string_pretty(
        &json!({ "hooks": hooks, "permissions": { "allow": allow_rules() } }),
    )
    .unwrap_or_default();
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

/// Hook entries an existing settings file lacks, as `event: command` lines.
pub(crate) fn missing_entries(existing: &str) -> Vec<String> {
    let parsed: Value = serde_json::from_str(existing).unwrap_or(Value::Null);
    entries()
        .into_iter()
        .filter(|(event, _, command)| !has_command(&parsed, event, command))
        .map(|(event, matcher, command)| match matcher {
            Some(matcher) => format!("{event} ({matcher}): {command}"),
            None => format!("{event}: {command}"),
        })
        .collect()
}

/// Hook entries the workspace settings file lacks, or all of them when it is absent.
pub(crate) fn workspace_missing(workspace: &Path) -> Vec<String> {
    let existing = std::fs::read_to_string(workspace.join(SETTINGS)).unwrap_or_default();
    missing_entries(&existing)
}

/// What setup did for the worktree pool, as report data.
pub(crate) struct Outcome {
    pub(crate) wrote: Vec<String>,
    pub(crate) missing: Vec<String>,
    pub(crate) missing_allow: Vec<String>,
}

/// Writes the hooks file when it is absent and never overwrites.
pub(crate) fn apply(workspace: &Path) -> io::Result<Outcome> {
    let mut outcome = Outcome {
        wrote: Vec::new(),
        missing: Vec::new(),
        missing_allow: Vec::new(),
    };
    let settings = workspace.join(SETTINGS);
    if settings.exists() {
        let existing = std::fs::read_to_string(&settings)?;
        outcome.missing = missing_entries(&existing);
        outcome.missing_allow = missing_allow(&existing);
    } else {
        if let Some(parent) = settings.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&settings, settings_text())?;
        outcome.wrote.push(SETTINGS.to_owned());
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    use dx_test_scratch::scratch;

    #[test]
    fn a_fresh_repository_gets_every_hook() {
        let dir = scratch("dx-worktree-setup-fresh-");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(outcome.wrote, vec![SETTINGS]);
        let settings = std::fs::read_to_string(dir.path().join(SETTINGS)).expect("settings");
        assert!(missing_entries(&settings).is_empty());
    }

    #[test]
    fn existing_files_are_never_overwritten() {
        let dir = scratch("dx-worktree-setup-existing-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        let own = "{\"hooks\": {}}\n";
        std::fs::write(dir.path().join(SETTINGS), own).expect("settings");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(SETTINGS)).expect("read"),
            own
        );
        assert!(outcome
            .missing
            .iter()
            .any(|entry| entry.contains("WorktreeCreate")));
        assert!(outcome
            .missing
            .iter()
            .any(|entry| entry.contains("PreToolUse (Bash)")));
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
        const LAUNCHER: &str = "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx ";
        let settings: Value = serde_json::from_str(&settings_text()).expect("settings json");
        let events = settings["hooks"].as_object().expect("hooks object");
        let total: usize = events
            .keys()
            .map(|event| commands(&settings, event).len())
            .sum();
        assert_eq!(total, HookKind::ALL.len());
        for kind in HookKind::ALL {
            let expected = match kind {
                HookKind::Create => "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook create",
                HookKind::Remove => "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook remove",
                HookKind::Stop => "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook stop",
                HookKind::Guard => "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook guard",
            };
            let (event, _) = hook_event(kind);
            assert_eq!(commands(&settings, event), vec![expected.to_owned()]);
            let argv: Vec<&str> = expected
                .strip_prefix(LAUNCHER)
                .expect("launcher prefix")
                .split_whitespace()
                .collect();
            assert_eq!(crate::exec::hook_kind(&argv), Some(kind));
        }
    }

    #[test]
    fn hook_commands_with_global_flags_count_as_missing() {
        let flagged = settings_text().replace("/tools/dx worktree", "/tools/dx --verbose worktree");
        assert_ne!(flagged, settings_text());
        assert_eq!(missing_entries(&flagged).len(), HookKind::ALL.len());
    }
}
