use std::path::PathBuf;

use serde_json::Value;

use super::git;
use super::host::Host;
use super::ops::{self, AddRequest};
use super::pool::{Pool, Who};
use super::WorktreeError;

fn field(input: &Value, name: &str) -> Option<String> {
    input
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Reads the JSON a hook gets on stdin.
pub(crate) fn parse(stdin: &str) -> Result<Value, WorktreeError> {
    serde_json::from_str(stdin)
        .map_err(|error| WorktreeError::usage("bad_hook_input", &format!("hook input: {error}")))
}

/// The directory a hook payload says it runs in.
pub(crate) fn cwd(input: &Value) -> Option<PathBuf> {
    field(input, "cwd").map(PathBuf::from)
}

/// The branch a named agent worktree uses.
pub(crate) fn branch_for(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!(
        "worktree-{}",
        cleaned.trim_matches(|c| c == '-' || c == '.')
    )
}

/// Leases a slot for a new agent worktree and returns its path.
pub(crate) fn create(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<PathBuf, WorktreeError> {
    let name = field(input, "name")
        .ok_or_else(|| WorktreeError::usage("bad_hook_input", "hook input has no name"))?;
    let branch = branch_for(&name);
    let create = if git::branch_exists(&pool.main, &branch) {
        None
    } else {
        Some(String::new())
    };
    let added = ops::add(pool, host, who, &AddRequest { branch, create })?;
    Ok(added.path)
}

/// Frees the slot of an agent worktree. Paths outside the pool are left alone.
pub(crate) fn remove(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<Option<PathBuf>, WorktreeError> {
    let path = field(input, "worktree_path")
        .map(PathBuf::from)
        .ok_or_else(|| WorktreeError::usage("bad_hook_input", "hook input has no worktree_path"))?;
    if pool.slot_of(&path).is_none() {
        return Ok(None);
    }
    let released = ops::release(pool, host, who, &path, false)?;
    Ok(Some(released.path))
}

fn is_git(word: &str) -> bool {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    name == "git" || name == "git.exe"
}

/// The line that blocks a `git worktree add|remove|prune` command, if the command is one.
pub(crate) fn guard(input: &Value) -> Option<String> {
    let command = input
        .get("tool_input")
        .and_then(|tool| tool.get("command"))
        .and_then(Value::as_str)?;
    for segment in command.split([';', '&', '|', '(', ')', '\n']) {
        let words: Vec<&str> = segment
            .split_whitespace()
            .skip_while(|word| word.contains('='))
            .collect();
        let Some((first, tail)) = words.split_first() else {
            continue;
        };
        if !is_git(first.trim_matches(|c| c == '"' || c == '\'')) {
            continue;
        }
        let mut rest = tail.iter().copied();
        let mut next = rest.next();
        while let Some(option) = next {
            if matches!(option, "-C" | "-c") {
                rest.next();
                next = rest.next();
            } else if option.starts_with('-') {
                next = rest.next();
            } else {
                break;
            }
        }
        if next != Some("worktree") {
            continue;
        }
        match rest.next() {
            Some("add") => {
                return Some(
                    "dx: use `tools/dx worktree add <branch>` instead of `git worktree add`"
                        .to_owned(),
                )
            }
            Some("remove") | Some("prune") => {
                return Some(
                    "dx: use `tools/dx worktree release` instead of `git worktree remove|prune`"
                        .to_owned(),
                )
            }
            _ => {}
        }
    }
    None
}
