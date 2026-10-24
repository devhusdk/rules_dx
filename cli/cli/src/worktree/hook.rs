use std::path::{Path, PathBuf};

use serde_json::Value;

use super::git;
use super::host::{Host, Server};
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

const RECYCLE_AFTER_SECS: u64 = 60;

fn in_use(servers: &[Server], cwds: Option<&[PathBuf]>, path: &Path) -> bool {
    let Some(cwds) = cwds else {
        return true;
    };
    if ops::server_in(servers, path) {
        return true;
    }
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    cwds.iter()
        .any(|cwd| cwd.starts_with(&real) || cwd.starts_with(path))
}

fn recycle_finished(pool: &Pool, host: &dyn Host, who: &Who) -> Vec<u32> {
    let Some(session) = who.session.as_deref() else {
        return Vec::new();
    };
    let candidates: Vec<_> = pool
        .records()
        .into_iter()
        .filter(|record| {
            record.held
                && record
                    .branch
                    .as_deref()
                    .is_some_and(|branch| branch.starts_with("worktree-"))
                && record.session.as_deref() == Some(session)
                && host.now().saturating_sub(record.leased_at) >= RECYCLE_AFTER_SECS
        })
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let servers = host.servers();
    let cwds = host.process_cwds();
    let mut freed = Vec::new();
    for record in candidates {
        let path = pool.slot_path(record.slot);
        if in_use(&servers, cwds.as_deref(), &path) {
            continue;
        }
        if ops::release(pool, host, who, &path, false).is_ok() {
            freed.push(record.slot);
        }
    }
    freed
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
    let freed = recycle_finished(pool, host, who);
    let create = if git::branch_exists(&pool.main, &branch) {
        None
    } else {
        Some(String::new())
    };
    let added = ops::add_except(pool, host, who, &AddRequest { branch, create }, &freed)?;
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

#[derive(Default)]
struct Commands {
    done: Vec<Vec<String>>,
    words: Vec<String>,
    word: Option<String>,
}

impl Commands {
    fn push(&mut self, c: char) {
        self.word.get_or_insert_with(String::new).push(c);
    }

    fn mark(&mut self) {
        self.word.get_or_insert_with(String::new);
    }

    fn end_word(&mut self) {
        if let Some(word) = self.word.take() {
            self.words.push(word);
        }
    }

    fn end_command(&mut self) {
        self.end_word();
        if !self.words.is_empty() {
            self.done.push(std::mem::take(&mut self.words));
        }
    }
}

fn skip_heredoc(chars: &[char], mut i: usize, tag: &str, strip: bool) -> usize {
    while i < chars.len() {
        let start = i;
        while i < chars.len() && chars[i] != '\n' {
            i += 1;
        }
        let line: String = chars[start..i].iter().collect();
        i = (i + 1).min(chars.len());
        let line = if strip {
            line.trim_start_matches('\t')
        } else {
            &line
        };
        if line == tag {
            break;
        }
    }
    i
}

fn closing_paren(chars: &[char], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < chars.len() {
        match chars[i] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    chars.len()
}

fn substitution(chars: &[char], i: usize, cmds: &mut Commands) -> Option<usize> {
    match chars[i] {
        '$' if chars.get(i + 1) == Some(&'(') && chars.get(i + 2) == Some(&'(') => {
            Some(closing_paren(chars, i + 1) + 1)
        }
        '$' if chars.get(i + 1) == Some(&'(') => {
            let close = closing_paren(chars, i + 1);
            let inner: String = chars[i + 2..close.max(i + 2)].iter().collect();
            cmds.done.extend(commands(&inner));
            Some(close + 1)
        }
        '`' => {
            let mut close = i + 1;
            while close < chars.len() && chars[close] != '`' {
                if chars[close] == '\\' {
                    close += 1;
                }
                close += 1;
            }
            let close = close.min(chars.len());
            let inner: String = chars[i + 1..close].iter().collect();
            cmds.done.extend(commands(&inner));
            Some(close + 1)
        }
        _ => None,
    }
}

fn commands(script: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = script.chars().collect();
    let mut cmds = Commands::default();
    let mut pending: Vec<(String, bool)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(next) = substitution(&chars, i, &mut cmds) {
            i = next;
            continue;
        }
        let c = chars[i];
        match c {
            '\'' => {
                cmds.mark();
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    cmds.push(chars[i]);
                    i += 1;
                }
            }
            '"' => {
                cmds.mark();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if let Some(next) = substitution(&chars, i, &mut cmds) {
                        i = next;
                        continue;
                    }
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    cmds.push(chars[i]);
                    i += 1;
                }
            }
            '\\' => {
                if i + 1 < chars.len() {
                    i += 1;
                    if chars[i] != '\n' {
                        cmds.push(chars[i]);
                    }
                }
            }
            '#' if cmds.word.is_none() => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '<' if chars.get(i + 1) == Some(&'<') && chars.get(i + 2) == Some(&'<') => {
                cmds.end_word();
                i += 3;
                continue;
            }
            '<' if chars.get(i + 1) == Some(&'<') => {
                cmds.end_word();
                i += 2;
                let strip = chars.get(i) == Some(&'-');
                if strip {
                    i += 1;
                }
                while chars.get(i).is_some_and(|c| *c == ' ' || *c == '\t') {
                    i += 1;
                }
                let mut tag = String::new();
                while let Some(&t) = chars.get(i) {
                    match t {
                        '\'' | '"' | '\\' => {}
                        _ if t.is_whitespace() || ";&|()<>".contains(t) => break,
                        _ => tag.push(t),
                    }
                    i += 1;
                }
                if !tag.is_empty() {
                    pending.push((tag, strip));
                }
                continue;
            }
            '\n' => {
                cmds.end_command();
                i += 1;
                for (tag, strip) in std::mem::take(&mut pending) {
                    i = skip_heredoc(&chars, i, &tag, strip);
                }
                continue;
            }
            ';' | '&' | '|' | '(' | ')' => cmds.end_command(),
            c if c.is_whitespace() => cmds.end_word(),
            c => cmds.push(c),
        }
        i += 1;
    }
    cmds.end_command();
    cmds.done
}

/// The line that blocks a `git worktree add|remove|prune` command, if the command is one.
pub(crate) fn guard(input: &Value) -> Option<String> {
    let command = input
        .get("tool_input")
        .and_then(|tool| tool.get("command"))
        .and_then(Value::as_str)?;
    for segment in commands(command) {
        let words: Vec<&str> = segment
            .iter()
            .map(String::as_str)
            .skip_while(|word| word.contains('='))
            .collect();
        let Some((first, tail)) = words.split_first() else {
            continue;
        };
        if !is_git(first) {
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
                    "dx: use `dx worktree add <branch>` instead of `git worktree add`".to_owned(),
                )
            }
            Some("remove") | Some("prune") => {
                return Some(
                    "dx: use `dx worktree release` instead of `git worktree remove|prune`"
                        .to_owned(),
                )
            }
            _ => {}
        }
    }
    None
}
