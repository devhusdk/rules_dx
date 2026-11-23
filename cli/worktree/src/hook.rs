use std::collections::HashSet;
use std::path::{Path, PathBuf};

use dx_process::shell::quote as shell_quote;
use serde_json::Value;

use super::WorktreeError;
use super::git;
use super::host::Host;
use super::ops::{self, AddRequest, Added, Released};
use super::pool::{HOLDER_ENV, Pool, Record, SESSION_ENV, Who};

fn field(input: &Value, name: &str) -> Option<String> {
    input
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Claude,
    Cursor,
    Gemini,
}

impl Dialect {
    pub fn of(input: &Value) -> Dialect {
        if input.get("hook_event_name").and_then(Value::as_str) == Some("BeforeTool") {
            Dialect::Gemini
        } else if input.get("cursor_version").is_some() || input.get("conversation_id").is_some() {
            Dialect::Cursor
        } else {
            Dialect::Claude
        }
    }

    fn session(self, input: &Value) -> Option<String> {
        match self {
            Dialect::Claude => None,
            Dialect::Cursor | Dialect::Gemini => {
                field(input, "session_id").or_else(|| field(input, "conversation_id"))
            }
        }
    }

    fn rewrite(self, updated: &Value) -> Value {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    Create,
    Remove,
    Stop,
    Guard,
    Bazel,
    Git,
}

impl HookKind {
    pub const ALL: [HookKind; 6] = [
        HookKind::Create,
        HookKind::Remove,
        HookKind::Stop,
        HookKind::Guard,
        HookKind::Bazel,
        HookKind::Git,
    ];

    #[must_use]
    pub fn from_word(word: &str) -> Option<HookKind> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            HookKind::Create => "create",
            HookKind::Remove => "remove",
            HookKind::Stop => "stop",
            HookKind::Guard => "guard",
            HookKind::Bazel => "bazel",
            HookKind::Git => "git",
        }
    }

    #[must_use]
    pub fn event(self) -> Option<&'static str> {
        match self {
            HookKind::Create => Some("WorktreeCreate"),
            HookKind::Remove => Some("WorktreeRemove"),
            HookKind::Stop => Some("SubagentStop"),
            HookKind::Guard => Some("PreToolUse"),
            HookKind::Bazel | HookKind::Git => None,
        }
    }

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
            kind.event().unwrap_or("agent")
        ),
    )
}

pub fn parse(stdin: &str) -> Result<Value, WorktreeError> {
    serde_json::from_str(stdin).map_err(|error| {
        WorktreeError::usage(
            "bad_hook_input",
            &format!("hook input: {error}; pass the agent's hook JSON on stdin"),
        )
    })
}

pub fn cwd(input: &Value) -> Option<PathBuf> {
    field(input, "cwd").map(PathBuf::from)
}

#[must_use]
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

fn free_branch(
    main: &Path,
    stem: &str,
    records: &[Record],
    checkouts: &[git::Checkout],
    remote: Option<&HashSet<String>>,
) -> String {
    let base = git::default_base(main).ok();
    (1..)
        .map(|n| {
            if n == 1 {
                stem.to_owned()
            } else {
                format!("{stem}-{n}")
            }
        })
        .find(|candidate| {
            if ops::held_on(records, candidate).is_some()
                || checkouts
                    .iter()
                    .any(|checkout| checkout.branch.as_deref() == Some(candidate.as_str()))
            {
                return false;
            }
            if git::branch_exists(main, candidate) {
                return base
                    .as_deref()
                    .is_none_or(|base| !git::branch_behind(main, candidate, base));
            }
            let on_origin = match remote {
                Some(set) => set.contains(candidate),
                None => git::ref_exists(main, &format!("origin/{candidate}")),
            };
            !on_origin
        })
        .unwrap_or_else(|| stem.to_owned())
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

pub fn create(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<Added, WorktreeError> {
    let name = field(input, "name").ok_or_else(|| missing(HookKind::Create, "name"))?;
    let freed = recycle_gone(pool, host, who);
    let warnings = ops::refresh(pool, host)?;
    let stem = branch_for(&name);
    ops::check_branch(pool, &stem)?;
    let has_origin = git::succeeds(&pool.main, &["remote", "get-url", "origin"]);
    let fetched = !warnings.iter().any(|w| w.code == "fetch_failed");
    let remote = if has_origin && fetched {
        match git::remote_heads(&pool.main, &stem, &|| host.interrupted()) {
            Ok(set) => Some(set),
            Err(_) => {
                if let Some(signo) = host.interrupted() {
                    return Err(WorktreeError::stopped(signo));
                } else {
                    None
                }
            }
        }
    } else {
        None
    };
    let lock = pool.lock()?;
    pool.forget_missing();
    let records = pool.records();
    let checkouts = pool.checkouts()?;
    let branch = free_branch(&pool.main, &stem, &records, &checkouts, remote.as_ref());
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
    ops::lease_locked(pool, host, &owner, &request, &freed, warnings, lock)
}

pub fn remove(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<Option<Released>, WorktreeError> {
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
    Ok(Some(released))
}

#[must_use]
pub fn ignored(kind: HookKind, input: &Value) -> bool {
    kind == HookKind::Stop
        && field(input, "agent_id").is_none()
        && Dialect::of(input) != Dialect::Claude
}

pub fn stop(
    pool: &Pool,
    host: &dyn Host,
    who: &Who,
    input: &Value,
) -> Result<Vec<u32>, WorktreeError> {
    if ignored(HookKind::Stop, input) {
        return Ok(Vec::new());
    }
    let agent = field(input, "agent_id").ok_or_else(|| missing(HookKind::Stop, "agent_id"))?;
    let session = who.session.clone().or_else(|| field(input, "session_id"));
    let lock = pool.lock()?;
    let mut marked = Vec::new();
    for mut record in pool.records() {
        if record.held
            && !record.gone
            && record.session == session
            && record.agent.as_deref() == Some(agent.as_str())
        {
            record.gone = true;
            record.stopped_at = Some(host.now());
            pool.write(&record)?;
            marked.push(record.slot);
        }
    }
    drop(lock);
    for slot in &marked {
        ops::stop_server(host, &pool.slot_path(*slot));
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
        other => prefix_command_values(other).and_then(plain),
    }
}

fn prefix_command_values(word: &str) -> Option<&'static [&'static str]> {
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
    words.get(i..).unwrap_or_default()
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
        while chars.get(i).is_some_and(|c| *c != '\n') {
            i += 1;
        }
        let line: String = chars.get(start..i).unwrap_or_default().iter().collect();
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
    for (i, c) in chars.iter().enumerate().skip(open) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    chars.len()
}

fn substitution(chars: &[char], i: usize, cmds: &mut Commands) -> Option<usize> {
    match chars.get(i)? {
        '$' if chars.get(i + 1) == Some(&'(') && chars.get(i + 2) == Some(&'(') => {
            Some(closing_paren(chars, i + 1) + 1)
        }
        '$' if chars.get(i + 1) == Some(&'(') => {
            let close = closing_paren(chars, i + 1);
            let inner: String = chars
                .get(i + 2..close.max(i + 2))
                .unwrap_or_default()
                .iter()
                .collect();
            cmds.done.extend(commands(&inner));
            Some(close + 1)
        }
        '`' => {
            let mut close = i + 1;
            while let Some(&c) = chars.get(close)
                && c != '`'
            {
                if c == '\\' {
                    close += 1;
                }
                close += 1;
            }
            let close = close.min(chars.len());
            let inner: String = chars.get(i + 1..close).unwrap_or_default().iter().collect();
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
    while let Some(&c) = chars.get(i) {
        if let Some(next) = substitution(&chars, i, &mut cmds) {
            i = next;
            continue;
        }
        match c {
            '\'' => {
                cmds.mark();
                i += 1;
                while let Some(&q) = chars.get(i)
                    && q != '\''
                {
                    cmds.push(q);
                    i += 1;
                }
            }
            '"' => {
                cmds.mark();
                i += 1;
                while let Some(&q) = chars.get(i)
                    && q != '"'
                {
                    if let Some(next) = substitution(&chars, i, &mut cmds) {
                        i = next;
                        continue;
                    }
                    let mut pushed = q;
                    if q == '\\'
                        && let Some(&escaped) = chars.get(i + 1)
                    {
                        i += 1;
                        pushed = escaped;
                    }
                    cmds.push(pushed);
                    i += 1;
                }
            }
            '\\' => {
                if let Some(&escaped) = chars.get(i + 1) {
                    i += 1;
                    if escaped != '\n' {
                        cmds.push(escaped);
                    }
                }
            }
            '#' if cmds.word.is_none() => {
                while chars.get(i).is_some_and(|q| *q != '\n') {
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
        if let Some(inner) = tail.get(1..).and_then(shell_script) {
            collect_segments(inner, depth + 1, out);
        }
    }
}

fn shell_script<'a>(args: &[&'a str]) -> Option<&'a str> {
    let mut command = false;
    let mut rest = args.iter();
    while let Some(&word) = rest.next() {
        if word == "--" || word == "-" {
            return rest.next().copied().filter(|_| command);
        }
        if matches!(word, "--rcfile" | "--init-file") {
            rest.next();
        } else if word.starts_with("--") {
            continue;
        } else if word.starts_with(['-', '+']) {
            command |= word.contains('c');
            for _ in word.matches(['o', 'O']) {
                rest.next();
            }
        } else {
            return command.then_some(word);
        }
    }
    None
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

fn deploys_with_apply(command: &str) -> bool {
    segments(command).iter().any(|segment| {
        let Some((first, rest)) = segment.split_first() else {
            return false;
        };
        if !matches!(base(first), "dx" | "dx.cmd" | "dx.exe") {
            return false;
        }
        let words: Vec<&str> = rest
            .iter()
            .map(String::as_str)
            .take_while(|word| *word != "--")
            .collect();
        words.contains(&"--apply") && words.iter().find(|w| !w.starts_with('-')) == Some(&"deploy")
    })
}

const DEPLOY_APPLY_ASK: &str = "dx deploy --apply publishes for real. Confirm with the user first.";

fn deploy_apply_deny(command: &str) -> String {
    let dry_run = command
        .split_whitespace()
        .filter(|word| *word != "--apply")
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "dx: `dx deploy --apply` publishes for real and needs a human or CI. Dry run with `{dry_run}` instead."
    )
}

#[derive(Debug, Clone, PartialEq)]
pub enum Guard {
    Allow,
    Block(String),
    Rewrite(Value),
    Ask(Value),
}

pub type LeaseLookup<'a> = &'a dyn Fn(&Path) -> Option<(PathBuf, Record)>;

const SLOT_SWITCHING: &[&str] = &[
    "checkout", "switch", "reset", "rebase", "fetch", "pull", "stash", "commit",
];

const DX_MUTATING: &[&str] = &["build", "test", "check"];

pub type SlotsLookup<'a> = &'a dyn Fn(&Path) -> Option<Vec<(PathBuf, Record)>>;

pub fn primary_checkout(input: &Value, who: &Who, slots: SlotsLookup) -> Option<String> {
    let command = input
        .get("tool_input")
        .and_then(|tool| tool.get("command"))
        .and_then(Value::as_str)?;
    let dirs = switching_dirs(command, cwd(input));
    if dirs.is_empty() {
        return None;
    }
    let caller = Who {
        session: who.session.clone().or_else(|| field(input, "session_id")),
        agent: field(input, "agent_id"),
        pid: who.pid,
    };
    dirs.iter().find_map(|dir| {
        let owned = slots(dir)?
            .into_iter()
            .find(|(_, record)| record.held && !record.broken && record.held_by(&caller))?;
        Some(format!(
            "{} is the primary checkout and you hold {}; run the command there, or dx worktree add <branch> for another slot",
            dir.display(),
            owned.0.display()
        ))
    })
}

pub fn guard(input: &Value, who: &Who, lease: LeaseLookup) -> Guard {
    guard_with(input, who, lease, &|name| std::env::var(name).ok())
}

pub fn guard_with(
    input: &Value,
    who: &Who,
    lease: LeaseLookup,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Guard {
    if let Some(line) = main_checkout_edit(input, env_get) {
        return Guard::Block(line);
    }
    if let Some(line) = blocked(input) {
        return Guard::Block(line);
    }
    if let Some(line) = foreign_slot(input, who, lease) {
        return Guard::Block(line);
    }
    let Some(tool) = input.get("tool_input").and_then(Value::as_object) else {
        return Guard::Allow;
    };
    let Some(command) = tool.get("command").and_then(Value::as_str) else {
        return Guard::Allow;
    };
    let dialect = Dialect::of(input);
    let deploy = deploys_with_apply(command);
    if deploy && (dialect != Dialect::Claude || codex_thread(env_get).is_some()) {
        return Guard::Block(deploy_apply_deny(command));
    }
    let mut exports = Vec::new();
    if let Some(session) = dialect.session(input)
        && runs_dx(command)
    {
        exports.push(format!("export {SESSION_ENV}={}", shell_quote(&session)));
    }
    let codex_thread = codex_thread(env_get);
    let subagent_thread = field(input, "session_id").filter(|id| {
        dialect == Dialect::Claude && codex_thread.as_deref().is_some_and(|t| t != id)
    });
    if let Some(agent) = field(input, "agent_id").or(subagent_thread)
        && runs_dx(command)
    {
        exports.push(format!("export {HOLDER_ENV}={}", shell_quote(&agent)));
    }
    if exports.is_empty() && !deploy {
        return Guard::Allow;
    }
    let mut updated = tool.clone();
    updated.insert(
        "command".to_owned(),
        Value::String(format!("{}\n{command}", exports.join("\n"))),
    );
    if deploy {
        let mut specific = serde_json::json!({
            "hookEventName": "PreToolUse",
            "permissionDecision": "ask",
            "permissionDecisionReason": DEPLOY_APPLY_ASK,
        });
        if !exports.is_empty()
            && let Some(object) = specific.as_object_mut()
        {
            object.insert("updatedInput".to_owned(), Value::Object(updated));
        }
        return Guard::Ask(serde_json::json!({ "hookSpecificOutput": specific }));
    }
    Guard::Rewrite(dialect.rewrite(&Value::Object(updated)))
}

fn main_checkout_edit(input: &Value, env_get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let tool = input.get("tool_input")?;
    let path = field(tool, "file_path").or_else(|| field(tool, "notebook_path"))?;
    let path = match cwd(input) {
        Some(dir) => dir.join(path),
        None => PathBuf::from(path),
    };
    super::primary::edit_refusal(&path, env_get).map(|reason| format!("dx: refused: {reason}"))
}

fn codex_thread(env_get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    env_get("CODEX_THREAD_ID").filter(|id| !id.is_empty())
}

fn foreign_slot(input: &Value, who: &Who, lease: LeaseLookup) -> Option<String> {
    let command = input
        .get("tool_input")
        .and_then(|tool| tool.get("command"))
        .and_then(Value::as_str)?;
    let dirs = switching_dirs(command, cwd(input));
    if dirs.is_empty() {
        return None;
    }
    let caller = Who {
        session: who.session.clone().or_else(|| field(input, "session_id")),
        agent: field(input, "agent_id"),
        pid: who.pid,
    };
    dirs.iter().find_map(|dir| {
        let (path, record) = lease(dir)?;
        record.refuses(&caller).then(|| {
            format!(
                "{} is leased to {}; run dx worktree add <branch> for your own slot",
                path.display(),
                record.holder(true).unwrap_or_default()
            )
        })
    })
}

fn switching_dirs(command: &str, start: Option<PathBuf>) -> Vec<PathBuf> {
    let mut current = start;
    let mut found = Vec::new();
    for segment in segments(command) {
        let Some((first, tail)) = segment.split_first() else {
            continue;
        };
        let words: Vec<&str> = tail.iter().map(String::as_str).collect();
        if base(first) == "cd" {
            let target = words
                .iter()
                .copied()
                .find(|word| !matches!(*word, "--" | "-L" | "-P" | "-e" | "-@"));
            current = target.and_then(|target| move_to(current.as_deref(), target));
            continue;
        }
        match base(first) {
            "bazel" | "bazelisk" => {
                found.extend(current.clone());
                continue;
            }
            "dx" | "dx.cmd" | "dx.exe" => {
                let sub = words.iter().copied().find(|word| !word.starts_with('-'));
                if sub.is_some_and(|sub| DX_MUTATING.contains(&sub)) {
                    found.extend(current.clone());
                }
                continue;
            }
            _ => {}
        }
        if !is_git(first) {
            continue;
        }
        let mut dir = current.clone();
        let mut rest = words.iter().copied();
        while let Some(option) = rest.next() {
            if option == "-C" {
                dir = rest.next().and_then(|value| move_to(dir.as_deref(), value));
            } else if option == "--git-dir"
                || option == "--work-tree"
                || option.starts_with("--git-dir=")
                || option.starts_with("--work-tree=")
            {
                dir = None;
                if !option.contains('=') {
                    rest.next();
                }
            } else if GIT_VALUES.contains(&option) {
                rest.next();
            } else if option.starts_with('-') {
            } else {
                if SLOT_SWITCHING.contains(&option) {
                    found.extend(dir);
                }
                break;
            }
        }
    }
    found
}

fn move_to(from: Option<&Path>, target: &str) -> Option<PathBuf> {
    if target == "-" || target.starts_with('~') || target.contains(['$', '`']) {
        return None;
    }
    Some(from?.join(target))
}

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
                );
            }
            Some("remove" | "prune") => {
                return Some(
                    "dx: use `dx worktree release` instead of `git worktree remove|prune`"
                        .to_owned(),
                );
            }
            _ => {}
        }
    }
    None
}
