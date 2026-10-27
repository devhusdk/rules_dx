use std::path::{Path, PathBuf};

use serde_json::Value;

use super::git;
use super::host::Host;
use super::ops::{self, AddRequest, Added};
use super::pool::{Pool, Who, HOLDER_ENV, SESSION_ENV};
use super::WorktreeError;

fn field(input: &Value, name: &str) -> Option<String> {
    input
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The agent whose hook payload a hook reads.
///
/// Codex sends the Claude Code shape. Cursor runs the hooks in
/// `.claude/settings.json` and sends its own fields. Gemini CLI names the
/// event `BeforeTool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Claude,
    Cursor,
    Gemini,
}

impl Dialect {
    /// The dialect of a hook payload.
    pub fn of(input: &Value) -> Dialect {
        if input.get("hook_event_name").and_then(Value::as_str) == Some("BeforeTool") {
            Dialect::Gemini
        } else if input.get("cursor_version").is_some() || input.get("conversation_id").is_some() {
            Dialect::Cursor
        } else {
            Dialect::Claude
        }
    }

    /// The session a payload names, when the agent's shells carry no session variable.
    fn session(self, input: &Value) -> Option<String> {
        match self {
            Dialect::Claude => None,
            Dialect::Cursor | Dialect::Gemini => {
                field(input, "session_id").or_else(|| field(input, "conversation_id"))
            }
        }
    }

    /// The hook output that replaces the tool input.
    fn rewrite(self, updated: Value) -> Value {
        match self {
            Dialect::Claude | Dialect::Cursor => serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "updatedInput": updated,
                }
            }),
            Dialect::Gemini => serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "BeforeTool",
                    "tool_input": updated,
                }
            }),
        }
    }
}

/// One agent hook that `dx worktree hook` serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    Create,
    Remove,
    Stop,
    Guard,
}

impl HookKind {
    /// Every hook kind, in the order help text lists them.
    pub const ALL: [HookKind; 4] = [
        HookKind::Create,
        HookKind::Remove,
        HookKind::Stop,
        HookKind::Guard,
    ];

    /// The hook kind a command-line word names.
    pub fn from_word(word: &str) -> Option<HookKind> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }

    /// The command-line word for this hook kind.
    pub fn word(self) -> &'static str {
        match self {
            HookKind::Create => "create",
            HookKind::Remove => "remove",
            HookKind::Stop => "stop",
            HookKind::Guard => "guard",
        }
    }

    /// The Claude Code event that runs this hook, for error text.
    pub fn event(self) -> &'static str {
        match self {
            HookKind::Create => "WorktreeCreate",
            HookKind::Remove => "WorktreeRemove",
            HookKind::Stop => "SubagentStop",
            HookKind::Guard => "PreToolUse",
        }
    }

    /// The hook words joined for a usage message.
    pub fn names() -> String {
        Self::ALL.map(HookKind::word).join(", ")
    }
}

fn missing(kind: HookKind, field: &str) -> WorktreeError {
    WorktreeError::usage(
        "bad_hook_input",
        &format!(
            "hook input has no {field}; run dx worktree hook {} from the {} hook",
            kind.word(),
            kind.event()
        ),
    )
}

/// Reads the JSON a hook gets on stdin.
pub fn parse(stdin: &str) -> Result<Value, WorktreeError> {
    serde_json::from_str(stdin).map_err(|error| {
        WorktreeError::usage(
            "bad_hook_input",
            &format!("hook input: {error}; pass the agent's hook JSON on stdin"),
        )
    })
}

/// The directory a hook payload says it runs in.
pub fn cwd(input: &Value) -> Option<PathBuf> {
    field(input, "cwd").map(PathBuf::from)
}

/// The branch a named agent worktree uses.
pub fn branch_for(name: &str) -> String {
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

/// The first agent branch name that is local or not on origin.
fn free_branch(main: &Path, name: &str) -> String {
    let stem = branch_for(name);
    (1..)
        .map(|n| {
            if n == 1 {
                stem.clone()
            } else {
                format!("{stem}-{n}")
            }
        })
        .find(|candidate| {
            git::branch_exists(main, candidate)
                || !git::ref_exists(main, &format!("origin/{candidate}"))
        })
        .unwrap_or(stem)
}

fn recycle_gone(pool: &Pool, host: &dyn Host, who: &Who) -> Vec<u32> {
    let Some(session) = who.session.as_deref() else {
        return Vec::new();
    };
    let mut freed = Vec::new();
    for record in pool.records() {
        let agent = record
            .branch
            .as_deref()
            .is_some_and(|branch| branch.starts_with("worktree-"));
        if record.held
            && agent
            && record.session.as_deref() == Some(session)
            && !ops::holder_alive(host, &record)
            && ops::release_lease(
                pool,
                host,
                &Who {
                    agent: record.agent.clone(),
                    ..who.clone()
                },
                &record,
            )
            .is_ok()
        {
            freed.push(record.slot);
        }
    }
    freed
}

/// Leases a slot for a new agent worktree.
pub fn create(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<Added, WorktreeError> {
    let name = field(input, "name").ok_or_else(|| missing(HookKind::Create, "name"))?;
    let freed = recycle_gone(pool, host, who);
    let warnings = ops::refresh(pool);
    let branch = free_branch(&pool.main, &name);
    ops::check_branch(pool, &branch)?;
    let owner = Who {
        agent: Some(branch.clone()),
        ..who.clone()
    };
    let request = AddRequest {
        branch,
        create: None,
        force: false,
        unclaimed: true,
    };
    ops::lease(pool, host, &owner, &request, &freed, warnings)
}

/// Frees the slot of an agent worktree. Paths outside the pool are left alone.
pub fn remove(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<Option<PathBuf>, WorktreeError> {
    let path = field(input, "worktree_path")
        .map(PathBuf::from)
        .ok_or_else(|| missing(HookKind::Remove, "worktree_path"))?;
    let Some(slot) = pool.slot_of(&path) else {
        return Ok(None);
    };
    let owner = Who {
        agent: pool.record(slot).and_then(|record| record.agent),
        ..who.clone()
    };
    let released = ops::release(pool, host, &owner, &path, false)?;
    Ok(Some(released.path))
}

/// Whether a hook has nothing to do for this payload.
///
/// Cursor runs `SubagentStop` without an `agent_id`; its subagents hold no slots.
pub fn ignored(kind: HookKind, input: &Value) -> bool {
    kind == HookKind::Stop
        && field(input, "agent_id").is_none()
        && Dialect::of(input) != Dialect::Claude
}

/// Marks the slots of a finished subagent as held by a gone holder.
pub fn stop(pool: &Pool, who: &Who, input: &Value) -> Result<Vec<u32>, WorktreeError> {
    if ignored(HookKind::Stop, input) {
        return Ok(Vec::new());
    }
    let agent = field(input, "agent_id").ok_or_else(|| missing(HookKind::Stop, "agent_id"))?;
    let session = who.session.clone().or_else(|| field(input, "session_id"));
    let _lock = pool.lock()?;
    let mut marked = Vec::new();
    for mut record in pool.records() {
        if record.held
            && !record.gone
            && record.session == session
            && record.agent.as_deref() == Some(agent.as_str())
        {
            record.gone = true;
            pool.write(&record)?;
            marked.push(record.slot);
        }
    }
    Ok(marked)
}

fn base(word: &str) -> &str {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    name.strip_suffix(".exe").unwrap_or(name)
}

fn is_git(word: &str) -> bool {
    base(word) == "git"
}

const KEYWORDS: &[&str] = &[
    "do", "then", "else", "elif", "if", "while", "until", "!", "{",
];

const GIT_VALUES: &[&str] = &[
    "-C",
    "-c",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--exec-path",
    "--config-env",
    "--super-prefix",
];

/// Options that take a value, and plain words that follow a wrapper's options.
struct Wrapper {
    values: &'static [&'static str],
    positionals: usize,
}

fn wrapper_values(word: &str) -> Option<Wrapper> {
    let plain = |values: &'static [&'static str]| {
        Some(Wrapper {
            values,
            positionals: 0,
        })
    };
    match base(word) {
        "timeout" => Some(Wrapper {
            values: &["-s", "-k", "--signal", "--kill-after"],
            positionals: 1,
        }),
        "nice" => plain(&["-n", "--adjustment"]),
        "stdbuf" => plain(&["-i", "-o", "-e", "--input", "--output", "--error"]),
        "setsid" => plain(&[]),
        "ionice" => plain(&["-c", "-n", "-p", "-P", "-u", "--class", "--classdata"]),
        other => legacy_values(other).and_then(plain),
    }
}

fn legacy_values(word: &str) -> Option<&'static [&'static str]> {
    match word {
        "env" => Some(&["-u", "-C", "-S", "--unset", "--chdir", "--split-string"]),
        "command" | "nohup" | "time" => Some(&[]),
        "exec" => Some(&["-a"]),
        "sudo" => Some(&[
            "-u",
            "-g",
            "-h",
            "-p",
            "-C",
            "-D",
            "-r",
            "-t",
            "-U",
            "-T",
            "--user",
            "--group",
            "--host",
            "--prompt",
            "--close-from",
            "--chdir",
            "--role",
            "--type",
            "--other-user",
            "--command-timeout",
        ]),
        "xargs" => Some(&[
            "-a",
            "-d",
            "-E",
            "-I",
            "-L",
            "-n",
            "-P",
            "-s",
            "--arg-file",
            "--delimiter",
            "--max-args",
            "--max-procs",
            "--max-chars",
        ]),
        _ => None,
    }
}

fn program<'a>(words: &'a [&'a str]) -> &'a [&'a str] {
    let mut i = 0;
    while let Some(word) = words.get(i) {
        if KEYWORDS.contains(word) || (word.contains('=') && !word.starts_with('-')) {
            i += 1;
            continue;
        }
        let Some(Wrapper {
            values,
            positionals,
        }) = wrapper_values(word)
        else {
            break;
        };
        i += 1;
        while let Some(option) = words.get(i) {
            if *option == "--" {
                i += 1;
                break;
            }
            if option.starts_with('-') && option.len() > 1 {
                i += if values.contains(option) { 2 } else { 1 };
            } else if option.contains('=') {
                i += 1;
            } else {
                break;
            }
        }
        for _ in 0..positionals {
            if words.get(i).is_some_and(|word| !word.starts_with('-')) {
                i += 1;
            }
        }
    }
    &words[i.min(words.len())..]
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

const SHELLS: &[&str] = &["sh", "bash", "zsh", "dash", "ksh"];
const MAX_SHELL_DEPTH: usize = 4;

/// Every simple command in `script` as its program and arguments, with wrappers skipped
/// and the scripts of `sh -c` and friends followed. Knows nothing about dx or git.
fn segments(script: &str) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    collect_segments(script, 0, &mut out);
    out
}

fn collect_segments(script: &str, depth: usize, out: &mut Vec<Vec<String>>) {
    for segment in commands(script) {
        let words: Vec<&str> = segment.iter().map(String::as_str).collect();
        let tail = program(&words);
        let Some(first) = tail.first() else {
            continue;
        };
        out.push(tail.iter().map(|word| (*word).to_owned()).collect());
        if depth >= MAX_SHELL_DEPTH || !SHELLS.contains(&base(first)) {
            continue;
        }
        let mut rest = tail[1..].iter();
        while let Some(word) = rest.next() {
            if matches!(*word, "-o" | "+o") {
                rest.next();
            } else if word.starts_with("--") {
                if *word == "--" {
                    break;
                }
            } else if word.starts_with('-') && word[1..].contains('c') {
                if let Some(inner) = rest.next() {
                    collect_segments(inner, depth + 1, out);
                }
                break;
            } else if !word.starts_with(['-', '+']) {
                break;
            }
        }
    }
}

fn runs_dx(command: &str) -> bool {
    segments(command).iter().any(|segment| {
        let Some((first, rest)) = segment.split_first() else {
            return false;
        };
        match base(first) {
            "dx" | "dx.cmd" | "dx.exe" => true,
            "bazel" | "bazelisk" => rest.iter().any(|word| word.ends_with(":dx")),
            _ => false,
        }
    })
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// What the Bash guard does with one command.
#[derive(Debug, Clone, PartialEq)]
pub enum Guard {
    Allow,
    Block(String),
    Rewrite(Value),
}

/// Blocks `git worktree add|remove|prune`, names the subagent for every dx command,
/// and names the session for dx when the agent's shells do not.
pub fn guard(input: &Value) -> Guard {
    if let Some(line) = blocked(input) {
        return Guard::Block(line);
    }
    let Some(tool) = input.get("tool_input").and_then(Value::as_object) else {
        return Guard::Allow;
    };
    let Some(command) = tool.get("command").and_then(Value::as_str) else {
        return Guard::Allow;
    };
    let dialect = Dialect::of(input);
    let mut exports = Vec::new();
    if let Some(session) = dialect.session(input) {
        if runs_dx(command) {
            exports.push(format!("export {SESSION_ENV}={}", shell_quote(&session)));
        }
    }
    if let Some(agent) = field(input, "agent_id") {
        if runs_dx(command) {
            exports.push(format!("export {HOLDER_ENV}={}", shell_quote(&agent)));
        }
    }
    if exports.is_empty() {
        return Guard::Allow;
    }
    let mut updated = tool.clone();
    updated.insert(
        "command".to_owned(),
        Value::String(format!("{}\n{command}", exports.join("\n"))),
    );
    Guard::Rewrite(dialect.rewrite(Value::Object(updated)))
}

/// The line that blocks a `git worktree add|remove|prune` command, if the command is one.
fn blocked(input: &Value) -> Option<String> {
    let command = input
        .get("tool_input")
        .and_then(|tool| tool.get("command"))
        .and_then(Value::as_str)?;
    for segment in segments(command) {
        let Some((first, tail)) = segment.split_first() else {
            continue;
        };
        let tail: Vec<&str> = tail.iter().map(String::as_str).collect();
        if !is_git(first) {
            continue;
        }
        let mut rest = tail.iter().copied();
        let mut next = rest.next();
        while let Some(option) = next {
            if GIT_VALUES.contains(&option) {
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
