use std::io;
use std::path::Path;

use serde_json::{json, Value};

const SETTINGS: &str = ".claude/settings.json";
const AGENTS: &str = "AGENTS.md";

/// The AGENTS.md lines that point agents at the pool.
pub(crate) const AGENT_LINES: [&str; 2] = [
    "- Start work with `tools/dx worktree add <branch>` and work in the path it prints.",
    "- When you finish, run `tools/dx worktree release`.",
];

fn hook_command(kind: &str) -> String {
    format!("tools/dx worktree hook {kind}")
}

fn entries() -> Vec<(&'static str, Option<&'static str>, String)> {
    vec![
        ("WorktreeCreate", None, hook_command("create")),
        ("WorktreeRemove", None, hook_command("remove")),
        ("PreToolUse", Some("Bash"), hook_command("guard")),
    ]
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

/// What `dx init --apply` did for the worktree pool, as report lines.
pub(crate) struct Outcome {
    pub(crate) wrote: Vec<String>,
    pub(crate) notes: Vec<String>,
}

/// Writes the hooks and AGENTS.md lines that are absent and never overwrites.
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
    let agents = workspace.join(AGENTS);
    let current = match std::fs::read_to_string(&agents) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let lacking: Vec<&str> = AGENT_LINES
        .iter()
        .copied()
        .filter(|line| current.as_deref().is_none_or(|text| !text.contains(line)))
        .collect();
    if !lacking.is_empty() {
        let mut text = current.unwrap_or_default();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        for line in lacking {
            text.push_str(line);
            text.push('\n');
        }
        std::fs::write(&agents, text)?;
        outcome.wrote.push(AGENTS.to_owned());
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    use dx_test_scratch::scratch;

    #[test]
    fn a_fresh_repository_gets_every_hook_and_both_lines() {
        let dir = scratch("dx-worktree-setup-fresh-");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(outcome.wrote, vec![SETTINGS, AGENTS]);
        let settings = std::fs::read_to_string(dir.path().join(SETTINGS)).expect("settings");
        assert!(missing_entries(&settings).is_empty());
        let agents = std::fs::read_to_string(dir.path().join(AGENTS)).expect("agents");
        assert!(agents.contains("worktree add") && agents.contains("worktree release"));
    }

    #[test]
    fn existing_files_are_never_overwritten() {
        let dir = scratch("dx-worktree-setup-existing-");
        std::fs::create_dir_all(dir.path().join(".claude")).expect("dir");
        let own = "{\"hooks\": {}}\n";
        std::fs::write(dir.path().join(SETTINGS), own).expect("settings");
        std::fs::write(dir.path().join(AGENTS), "# Mine\n").expect("agents");
        let outcome = apply(dir.path()).expect("apply");
        assert_eq!(
            std::fs::read_to_string(dir.path().join(SETTINGS)).expect("read"),
            own
        );
        assert_eq!(outcome.notes.len(), 1);
        assert!(outcome.notes[0].contains("WorktreeCreate"));
        assert!(outcome.notes[0].contains("PreToolUse (Bash)"));
        let agents = std::fs::read_to_string(dir.path().join(AGENTS)).expect("agents");
        assert!(agents.starts_with("# Mine\n"));
        assert!(agents.contains("worktree release"));
    }

    #[test]
    fn a_second_apply_changes_nothing() {
        let dir = scratch("dx-worktree-setup-twice-");
        apply(dir.path()).expect("first");
        let before = std::fs::read_to_string(dir.path().join(AGENTS)).expect("agents");
        let outcome = apply(dir.path()).expect("second");
        assert!(outcome.wrote.is_empty() && outcome.notes.is_empty());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(AGENTS)).expect("agents"),
            before
        );
    }
}
