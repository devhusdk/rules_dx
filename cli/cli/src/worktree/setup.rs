use std::io;
use std::path::Path;

use serde_json::{json, Value};

use super::HookKind;

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

/// The settings file a fresh repository gets.
pub(crate) fn settings_text() -> String {
    let mut hooks = serde_json::Map::new();
    for (event, matcher, command) in entries() {
        hooks.insert(event.to_owned(), json!([entry_value(matcher, &command)]));
    }
    let text = serde_json::to_string_pretty(&json!({ "hooks": hooks })).unwrap_or_default();
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

/// What `dx init --apply` did for the worktree pool, as report lines.
pub(crate) struct Outcome {
    pub(crate) wrote: Vec<String>,
    pub(crate) notes: Vec<String>,
}

/// Writes the hooks file when it is absent and never overwrites.
pub(crate) fn apply(workspace: &Path) -> io::Result<Outcome> {
    let mut outcome = Outcome {
        wrote: Vec::new(),
        notes: Vec::new(),
    };
    let settings = workspace.join(SETTINGS);
    if settings.exists() {
        let existing = std::fs::read_to_string(&settings)?;
        let missing = missing_entries(&existing);
        if !missing.is_empty() {
            outcome.notes.push(format!(
                "dx: {SETTINGS} exists; add these hooks: {}",
                missing.join("; ")
            ));
        }
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
        assert_eq!(outcome.notes.len(), 1);
        assert!(outcome.notes[0].contains("WorktreeCreate"));
        assert!(outcome.notes[0].contains("PreToolUse (Bash)"));
    }

    #[test]
    fn a_second_apply_changes_nothing() {
        let dir = scratch("dx-worktree-setup-twice-");
        apply(dir.path()).expect("first");
        let outcome = apply(dir.path()).expect("second");
        assert!(outcome.wrote.is_empty() && outcome.notes.is_empty());
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
