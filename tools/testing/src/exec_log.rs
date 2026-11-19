//! Reader for Bazel's `--execution_log_compact_file`, a comparison of two logs, and a diff that
//! names why spawns missed the cache.
//!
//! The log is a zstd stream of varint-length `ExecLogEntry` messages. Every spawn output
//! becomes the key `<label> <mnemonic> <output path>`, valued with the file digest, the sorted
//! child digests of a directory, or the target of an unresolved symlink.
//!
//! The fixtures in `exec_log_fixtures` were recorded with Bazel 9.3.1 from a scratch workspace
//! holding `.bazelversion` (`9.3.1`) and an empty `MODULE.bazel`.
//!
//! `first` and `second` come from a `BUILD.bazel` with two genrules: `stable` (`echo hi > $@`)
//! and `stamped` (`date +%s%N > $@`):
//!
//! ```text
//! bazel --output_base=$PWD/ob_first --bazelrc=/dev/null build --noenable_workspace \
//!   --remote_cache= --remote_executor= --disk_cache= \
//!   --execution_log_compact_file=$PWD/first.log.zst //:stable //:stamped
//! bazel --output_base=$PWD/ob_second --bazelrc=/dev/null build --noenable_workspace \
//!   --remote_cache= --remote_executor= --disk_cache= \
//!   --execution_log_compact_file=$PWD/second.log.zst //:stable //:stamped
//! ```
//!
//! `cache_first` and `cache_second` come from three genrules that each run `cat $< > $@`:
//! `same` on `same.txt` (`fixed`), `changed` on `changed.txt`, and `nocache` on `same.txt`
//! with `tags = ["no-cache"]`. `changed.txt` holds `old` for the first run and `new` for the
//! second, and both runs share one disk cache:
//!
//! ```text
//! bazel --output_base=$PWD/ob_old --bazelrc=/dev/null build --noenable_workspace \
//!   --remote_cache= --remote_executor= --disk_cache=$PWD/dc \
//!   --execution_log_compact_file=$PWD/cache_first.log.zst //:same //:changed //:nocache
//! bazel --output_base=$PWD/ob_new --bazelrc=/dev/null build --noenable_workspace \
//!   --remote_cache= --remote_executor= --disk_cache=$PWD/dc \
//!   --execution_log_compact_file=$PWD/cache_second.log.zst //:same //:changed //:nocache
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

pub type Log = BTreeMap<String, String>;

enum Node {
    File {
        path: String,
        hash: String,
    },
    Directory {
        path: String,
        files: Vec<(String, String)>,
    },
    Symlink {
        path: String,
        target: String,
    },
    Set {
        direct: Vec<u64>,
        transitive: Vec<u64>,
    },
    Runfiles {
        path: String,
        set: u64,
        repo_mapping: Option<String>,
    },
}

impl Node {
    fn output(&self) -> Option<(&str, String)> {
        match self {
            Self::File { path, hash } => Some((path, hash.clone())),
            Self::Directory { path, files } => {
                let mut hashes: Vec<&str> = files.iter().map(|(_, hash)| hash.as_str()).collect();
                hashes.sort_unstable();
                Some((path, hashes.join(" ")))
            }
            Self::Symlink { path, target } => Some((path, target.clone())),
            Self::Set { .. } | Self::Runfiles { .. } => None,
        }
    }
}

/// One spawn of an execution log.
#[derive(Debug, Default)]
pub struct Spawn {
    pub label: String,
    pub mnemonic: String,
    pub runner: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cache_hit: bool,
    pub remote_cacheable: bool,
    pub digest: String,
    pub total_millis: u64,
    input_set: u64,
    tool_set: u64,
    outputs: Vec<u64>,
}

/// A parsed execution log: its spawns plus the files and input sets they point at.
#[derive(Default)]
pub struct ExecLog {
    nodes: BTreeMap<u64, Node>,
    pub spawns: Vec<Spawn>,
}

struct Fields<'a> {
    bytes: &'a [u8],
}

enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}
fn varint(bytes: &mut &[u8]) -> Result<u64, String> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = bytes.split_first().ok_or("truncated varint")?;
        *bytes = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("varint longer than 10 bytes".to_owned())
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], String> {
    if bytes.len() < len {
        return Err(format!("need {len} bytes, have {}", bytes.len()));
    }
    let (head, rest) = bytes.split_at(len);
    *bytes = rest;
    Ok(head)
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u64, Value<'a>), String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes.is_empty() {
            return None;
        }
        Some(self.read())
    }
}

impl<'a> Fields<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn read(&mut self) -> Result<(u64, Value<'a>), String> {
        let tag = varint(&mut self.bytes)?;
        let value = match tag & 7 {
            0 => Value::Varint(varint(&mut self.bytes)?),
            1 => {
                take(&mut self.bytes, 8)?;
                Value::Varint(0)
            }
            2 => {
                let len = usize::try_from(varint(&mut self.bytes)?).map_err(|e| e.to_string())?;
                Value::Bytes(take(&mut self.bytes, len)?)
            }
            5 => {
                take(&mut self.bytes, 4)?;
                Value::Varint(0)
            }
            wire => return Err(format!("unsupported wire type {wire}")),
        };
        Ok((tag >> 3, value))
    }
}

fn text(bytes: &[u8]) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string())
}

fn digest_hash(bytes: &[u8]) -> Result<String, String> {
    let mut hash = String::new();
    for field in Fields::new(bytes) {
        if let (1, Value::Bytes(value)) = field? {
            hash = text(value)?;
        }
    }
    Ok(hash)
}

fn file(bytes: &[u8]) -> Result<(String, String), String> {
    let (mut path, mut hash) = (String::new(), String::new());
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(value)) => path = text(value)?,
            (2, Value::Bytes(value)) => hash = digest_hash(value)?,
            _ => {}
        }
    }
    Ok((path, hash))
}

fn directory(bytes: &[u8]) -> Result<Node, String> {
    let (mut path, mut files) = (String::new(), Vec::new());
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(value)) => path = text(value)?,
            (2, Value::Bytes(value)) => files.push(file(value)?),
            _ => {}
        }
    }
    Ok(Node::Directory { path, files })
}

fn symlink(bytes: &[u8]) -> Result<Node, String> {
    let (mut path, mut target) = (String::new(), String::new());
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(value)) => path = text(value)?,
            (2, Value::Bytes(value)) => target = text(value)?,
            _ => {}
        }
    }
    Ok(Node::Symlink { path, target })
}

/// Reads a repeated uint32 field, which protobuf may write packed or one value per tag.
fn ids(value: Value<'_>, into: &mut Vec<u64>) -> Result<(), String> {
    match value {
        Value::Varint(id) => into.push(id),
        Value::Bytes(mut packed) => {
            while !packed.is_empty() {
                into.push(varint(&mut packed)?);
            }
        }
    }
    Ok(())
}

fn input_set(bytes: &[u8]) -> Result<Node, String> {
    let (mut direct, mut transitive) = (Vec::new(), Vec::new());
    for field in Fields::new(bytes) {
        match field? {
            (4, value) => ids(value, &mut transitive)?,
            (5, value) => ids(value, &mut direct)?,
            _ => {}
        }
    }
    Ok(Node::Set { direct, transitive })
}

fn runfiles(bytes: &[u8]) -> Result<Node, String> {
    let (mut path, mut set, mut repo_mapping) = (String::new(), 0, None);
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(value)) => path = text(value)?,
            (2, Value::Varint(id)) => set = id,
            (6, Value::Bytes(value)) => repo_mapping = Some(file(value)?.1),
            _ => {}
        }
    }
    Ok(Node::Runfiles {
        path,
        set,
        repo_mapping,
    })
}

fn pair(bytes: &[u8]) -> Result<(String, String), String> {
    let (mut name, mut value) = (String::new(), String::new());
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(bytes)) => name = text(bytes)?,
            (2, Value::Bytes(bytes)) => value = text(bytes)?,
            _ => {}
        }
    }
    Ok((name, value))
}

fn total_millis(metrics: &[u8]) -> Result<u64, String> {
    let mut millis = 0;
    for field in Fields::new(metrics) {
        if let (1, Value::Bytes(duration)) = field? {
            for part in Fields::new(duration) {
                match part? {
                    (1, Value::Varint(seconds)) => millis += seconds.saturating_mul(1000),
                    (2, Value::Varint(nanos)) => millis += nanos / 1_000_000,
                    _ => {}
                }
            }
        }
    }
    Ok(millis)
}

fn spawn(bytes: &[u8]) -> Result<Spawn, String> {
    let mut found = Spawn::default();
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(value)) => found.args.push(text(value)?),
            (2, Value::Bytes(value)) => {
                let (name, value) = pair(value)?;
                found.env.insert(name, value);
            }
            (4, Value::Varint(id)) => found.input_set = id,
            (5, Value::Varint(id)) => found.tool_set = id,
            (6, Value::Bytes(output)) => {
                for inner in Fields::new(output) {
                    if let (5, Value::Varint(id)) = inner? {
                        found.outputs.push(id);
                    }
                }
            }
            (7, Value::Bytes(value)) => found.label = text(value)?,
            (8, Value::Bytes(value)) => found.mnemonic = text(value)?,
            (11, Value::Bytes(value)) => found.runner = text(value)?,
            (12, Value::Varint(value)) => found.cache_hit = value != 0,
            (15, Value::Varint(value)) => found.remote_cacheable = value != 0,
            (16, Value::Bytes(value)) => found.digest = digest_hash(value)?,
            (18, Value::Bytes(value)) => found.total_millis = total_millis(value)?,
            _ => {}
        }
    }
    Ok(found)
}

impl ExecLog {
    /// Parses a compact execution log.
    ///
    /// # Errors
    ///
    /// Returns `Err` when the bytes are not a zstd stream of log entries.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder =
            ruzstd::decoding::StreamingDecoder::new(bytes).map_err(|e| e.to_string())?;
        let mut raw = Vec::new();
        decoder.read_to_end(&mut raw).map_err(|e| e.to_string())?;

        let mut log = Self::default();
        let mut rest = raw.as_slice();
        while !rest.is_empty() {
            let len = usize::try_from(varint(&mut rest)?).map_err(|e| e.to_string())?;
            let entry = take(&mut rest, len)?;
            let mut id = 0;
            let mut node = None;
            for field in Fields::new(entry) {
                match field? {
                    (1, Value::Varint(value)) => id = value,
                    (3, Value::Bytes(value)) => {
                        let (path, hash) = file(value)?;
                        node = Some(Node::File { path, hash });
                    }
                    (4, Value::Bytes(value)) => node = Some(directory(value)?),
                    (5, Value::Bytes(value)) => node = Some(symlink(value)?),
                    (6, Value::Bytes(value)) => node = Some(input_set(value)?),
                    (7, Value::Bytes(value)) => log.spawns.push(spawn(value)?),
                    (10, Value::Bytes(value)) => node = Some(runfiles(value)?),
                    _ => {}
                }
            }
            if let Some(node) = node {
                log.nodes.insert(id, node);
            }
        }
        Ok(log)
    }

    /// Returns each output path of a spawn with its digest.
    ///
    /// # Errors
    ///
    /// Returns `Err` when an output id has no file, directory or symlink entry.
    pub fn outputs(&self, spawn: &Spawn) -> Result<Vec<(String, String)>, String> {
        spawn
            .outputs
            .iter()
            .map(|id| {
                self.nodes
                    .get(id)
                    .and_then(Node::output)
                    .map(|(path, value)| (path.to_owned(), value))
                    .ok_or_else(|| format!("spawn output {id} has no entry"))
            })
            .collect()
    }

    /// Returns every input and tool file of a spawn, keyed by path, with directories expanded.
    ///
    /// # Errors
    ///
    /// Returns `Err` when an input id has no entry.
    pub fn inputs(&self, spawn: &Spawn) -> Result<BTreeMap<String, String>, String> {
        let mut found = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut pending: Vec<u64> = [spawn.input_set, spawn.tool_set]
            .into_iter()
            .filter(|id| *id != 0)
            .collect();
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            match self.nodes.get(&id) {
                Some(Node::Set { direct, transitive }) => {
                    pending.extend(direct.iter().chain(transitive));
                }
                Some(Node::File { path, hash }) => {
                    found.insert(path.clone(), hash.clone());
                }
                Some(Node::Directory { path, files }) if !files.is_empty() => {
                    for (child, hash) in files {
                        let child = if child.starts_with(&format!("{path}/")) {
                            child.clone()
                        } else {
                            format!("{path}/{child}")
                        };
                        found.insert(child, hash.clone());
                    }
                }
                Some(Node::Directory { path, .. }) => {
                    found.insert(path.clone(), String::new());
                }
                Some(Node::Symlink { path, target }) => {
                    found.insert(path.clone(), format!("-> {target}"));
                }
                // The tree's files are keyed by their exec path; the manifest sits in the tree.
                Some(Node::Runfiles {
                    path,
                    set,
                    repo_mapping,
                }) => {
                    pending.push(*set);
                    if let Some(hash) = repo_mapping {
                        found.insert(format!("{path}/_repo_mapping"), hash.clone());
                    }
                }
                None => return Err(format!("spawn input {id} has no entry")),
            }
        }
        Ok(found)
    }

    /// Returns `<label> <mnemonic> <first output path>`, which names a spawn across runs.
    #[must_use]
    pub fn key(&self, spawn: &Spawn) -> String {
        let path = spawn
            .outputs
            .first()
            .and_then(|id| self.nodes.get(id))
            .and_then(Node::output)
            .map_or("", |(path, _)| path);
        format!("{} {} {path}", spawn.label, spawn.mnemonic)
    }
}

/// Parses an execution log into its output keys and digests.
///
/// # Errors
///
/// Returns `Err` when the log cannot be parsed or a spawn output has no entry.
pub fn read(bytes: &[u8]) -> Result<Log, String> {
    let parsed = ExecLog::parse(bytes)?;
    let mut log = Log::new();
    for spawn in &parsed.spawns {
        for (path, value) in parsed.outputs(spawn)? {
            log.insert(format!("{} {} {path}", spawn.label, spawn.mnemonic), value);
        }
    }
    Ok(log)
}

/// Most differing inputs one spawn lists before it only counts the rest.
const INPUT_LINES: usize = 20;

fn ran_here(spawn: &Spawn) -> bool {
    !spawn.cache_hit && spawn.runner != "remote"
}

fn changed(
    out: &mut Vec<String>,
    what: &str,
    first: &BTreeMap<String, &str>,
    second: &BTreeMap<String, &str>,
) {
    for (name, value) in second {
        match first.get(name) {
            Some(old) if old == value => {}
            Some(old) => out.push(format!("{what} {name}: {old} -> {value}")),
            None => out.push(format!("{what} {name}: only in the second log")),
        }
    }
    for name in first.keys().filter(|name| !second.contains_key(*name)) {
        out.push(format!("{what} {name}: only in the first log"));
    }
}

fn view(map: &BTreeMap<String, String>) -> BTreeMap<String, &str> {
    map.iter().map(|(k, v)| (k.clone(), v.as_str())).collect()
}

/// Keys each arg by its zero-padded position, so a changed arg shows where it sits.
fn args(spawn: &Spawn) -> BTreeMap<String, &str> {
    spawn
        .args
        .iter()
        .enumerate()
        .map(|(at, arg)| (format!("{at:03}"), arg.as_str()))
        .collect()
}

fn spawn_changes(
    first: &ExecLog,
    old: &Spawn,
    second: &ExecLog,
    new: &Spawn,
) -> Result<Vec<String>, String> {
    let (old_inputs, new_inputs) = (first.inputs(old)?, second.inputs(new)?);
    let mut inputs = Vec::new();
    changed(&mut inputs, "input", &view(&old_inputs), &view(&new_inputs));
    let mut lines: Vec<String> = inputs.iter().take(INPUT_LINES).cloned().collect();
    if inputs.len() > INPUT_LINES {
        lines.push(format!("{} more inputs differ", inputs.len() - INPUT_LINES));
    }
    changed(&mut lines, "arg", &args(old), &args(new));
    changed(&mut lines, "env", &view(&old.env), &view(&new.env));
    Ok(lines)
}

/// Names why the slowest spawns that ran in the second log missed the cache, against the first.
///
/// # Errors
///
/// Returns `Err` when a spawn points at an input with no entry.
pub fn diff(first: &ExecLog, second: &ExecLog, top: usize) -> Result<Vec<String>, String> {
    let mut earlier: BTreeMap<String, &Spawn> = BTreeMap::new();
    for spawn in &first.spawns {
        earlier.entry(first.key(spawn)).or_insert(spawn);
    }
    let mut slow: Vec<&Spawn> = second.spawns.iter().filter(|s| ran_here(s)).collect();
    slow.sort_by_key(|spawn| std::cmp::Reverse(spawn.total_millis));
    let mut out = Vec::new();
    for spawn in slow.into_iter().take(top) {
        let key = second.key(spawn);
        let seconds = spawn.total_millis / 1000;
        let tenths = spawn.total_millis % 1000 / 100;
        out.push(format!("{seconds}.{tenths}s {key} ({})", spawn.runner));
        let reasons = if !spawn.remote_cacheable {
            vec!["not remote cacheable".to_owned()]
        } else if let Some(old) = earlier.get(&key) {
            // Bazel records no digest for some spawns, such as test runs.
            let digested = !old.digest.is_empty() && !spawn.digest.is_empty();
            let found = if digested && old.digest == spawn.digest {
                Vec::new()
            } else {
                spawn_changes(first, old, second, spawn)?
            };
            if !found.is_empty() {
                found
            } else if digested && old.digest != spawn.digest {
                vec!["digest differs, but inputs, args and env match".to_owned()]
            } else {
                vec![format!(
                    "same action as in the first log ({}): its result never reached the cache",
                    old.runner
                )]
            }
        } else {
            vec!["no spawn with this key in the first log".to_owned()]
        };
        out.extend(reasons.into_iter().map(|reason| format!("  {reason}")));
    }
    Ok(out)
}

/// Parses an allowlist file into its entries.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn parse_allowlist(text: &str) -> Result<BTreeSet<String>, String> {
    let mut keys = BTreeSet::new();
    let mut previous: Option<&str> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let label_ok = fields
            .first()
            .is_some_and(|label| label.starts_with("//") || label.starts_with('@'));
        if fields.len() != 3 || fields.iter().any(|f| f.is_empty()) || !label_ok {
            return Err(format!("`{line}` is not `<label> <mnemonic> <path>`"));
        }
        if previous.is_some_and(|p| p >= line) {
            return Err(format!("`{line}` is not sorted or is repeated"));
        }
        previous = Some(line);
        keys.insert(line.to_owned());
    }
    Ok(keys)
}

#[must_use]
pub fn problems(first: &Log, second: &Log, allowlist: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for (key, value) in first {
        match second.get(key) {
            None => out.push(format!("`{key}` is only in the first log")),
            Some(other) if other != value => {
                if !allowlist.contains(key) {
                    out.push(format!("`{key}` differs between the logs"));
                }
            }
            Some(_) => {
                if allowlist.contains(key) {
                    out.push(format!("`{key}` no longer differs: delete the line"));
                }
            }
        }
    }
    for key in second.keys().filter(|key| !first.contains_key(*key)) {
        out.push(format!("`{key}` is only in the second log"));
    }
    for key in allowlist {
        if !first.contains_key(key) && !second.contains_key(key) {
            out.push(format!("`{key}` is in neither log: delete the line"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: &[u8] = include_bytes!("../exec_log_fixtures/first.log.zst");
    const SECOND: &[u8] = include_bytes!("../exec_log_fixtures/second.log.zst");
    const STABLE: &str = "//:stable Genrule bazel-out/k8-fastbuild/bin/stable.txt";
    const STAMPED: &str = "//:stamped Genrule bazel-out/k8-fastbuild/bin/stamped.txt";

    fn logs() -> (Log, Log) {
        (read(FIRST).unwrap(), read(SECOND).unwrap())
    }

    fn allow(text: &str) -> BTreeSet<String> {
        parse_allowlist(text).unwrap()
    }

    fn field(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![tag, u8::try_from(body.len()).unwrap()];
        out.extend_from_slice(body);
        out
    }

    #[test]
    fn both_logs_hold_both_genrules() {
        let (first, second) = logs();
        for log in [&first, &second] {
            assert_eq!(
                log.keys().map(String::as_str).collect::<Vec<_>>(),
                [STABLE, STAMPED]
            );
        }
        assert_eq!(first[STABLE], second[STABLE]);
        assert_ne!(first[STAMPED], second[STAMPED]);
    }

    #[test]
    fn the_time_dependent_rule_needs_a_line() {
        let (first, second) = logs();
        let found = problems(&first, &second, &allow(""));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains(STAMPED) && found[0].contains("differs"));
    }

    #[test]
    fn a_line_clears_the_difference() {
        let (first, second) = logs();
        let line = format!("{STAMPED}\n");
        assert_eq!(
            problems(&first, &second, &allow(&line)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn a_line_for_a_matching_rule_is_stale() {
        let (first, second) = logs();
        let lines = format!("{STABLE}\n{STAMPED}\n");
        let found = problems(&first, &second, &allow(&lines));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains(STABLE) && found[0].contains("delete the line"));
    }

    #[test]
    fn a_key_in_one_log_is_reported() {
        let (first, mut second) = logs();
        second.remove(STABLE);
        let found = problems(&first, &second, &allow(&format!("{STAMPED}\n")));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("only in the first log"));
        let found = problems(&second, &first, &allow(&format!("{STAMPED}\n")));
        assert!(found[0].contains("only in the second log"), "{found:?}");
    }

    #[test]
    fn a_line_for_an_unknown_key_is_stale() {
        let (first, second) = logs();
        let lines = format!("//:gone Genrule x\n{STAMPED}\n");
        let found = problems(&first, &second, &allow(&lines));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("neither log"));
    }

    #[test]
    fn bad_allowlists_are_rejected() {
        assert_eq!(allow(""), BTreeSet::new());
        assert!(parse_allowlist("//:a M p\n@r//:b M p\n").is_ok());
        for (text, needle) in [
            ("//:a M\n", "is not `<label> <mnemonic> <path>`"),
            ("//:a M p q\n", "is not `<label> <mnemonic> <path>`"),
            ("//:a  M p\n", "is not `<label> <mnemonic> <path>`"),
            (":a M p\n", "is not `<label> <mnemonic> <path>`"),
            ("//:b M p\n//:a M p\n", "not sorted"),
            ("//:a M p\n//:a M p\n", "not sorted"),
            ("\n", "is not `<label> <mnemonic> <path>`"),
        ] {
            let err = parse_allowlist(text).unwrap_err();
            assert!(err.contains(needle), "{text:?}: {err}");
        }
    }

    #[test]
    fn directory_outputs_are_keyed_by_sorted_child_hashes() {
        let child = |path: &str, hash: &str| {
            let digest = field(0x0a, hash.as_bytes());
            [field(0x0a, path.as_bytes()), field(0x12, &digest)].concat()
        };
        let dir = [
            field(0x0a, b"out/d"),
            field(0x12, &child("out/d/b", "bb")),
            field(0x12, &child("out/d/a", "cc")),
        ]
        .concat();
        let spawn = [
            field(0x32, &[0x28, 0x01]),
            field(0x3a, b"//:d"),
            field(0x42, b"M"),
        ]
        .concat();
        let first = [vec![0x08, 0x01], field(0x22, &dir)].concat();
        let second = field(0x3a, &spawn);
        let raw = [
            vec![u8::try_from(first.len()).unwrap()],
            first,
            vec![u8::try_from(second.len()).unwrap()],
            second,
        ]
        .concat();
        let zst = ruzstd::encoding::compress_to_vec(
            raw.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        let log = read(&zst).unwrap();
        assert_eq!(log["//:d M out/d"], "bb cc");
    }

    const CACHE_FIRST: &[u8] = include_bytes!("../exec_log_fixtures/cache_first.log.zst");
    const CACHE_SECOND: &[u8] = include_bytes!("../exec_log_fixtures/cache_second.log.zst");
    const CHANGED: &str = "//:changed Genrule bazel-out/k8-fastbuild/bin/changed.out";
    const NOCACHE: &str = "//:nocache Genrule bazel-out/k8-fastbuild/bin/nocache.out";
    const SAME: &str = "//:same Genrule bazel-out/k8-fastbuild/bin/same.out";
    const OLD: &str = "01d09d19c2139a46aebfb577780d123d7396e97201bc7ead210a2ebff8239dee";

    fn cache_logs() -> (ExecLog, ExecLog) {
        (
            ExecLog::parse(CACHE_FIRST).unwrap(),
            ExecLog::parse(CACHE_SECOND).unwrap(),
        )
    }

    fn by_key<'a>(log: &'a ExecLog, key: &str) -> &'a Spawn {
        log.spawns.iter().find(|s| log.key(s) == key).unwrap()
    }

    #[test]
    fn spawns_carry_their_cache_fields() {
        let (_, second) = cache_logs();
        let same = by_key(&second, SAME);
        assert!(same.cache_hit && same.remote_cacheable, "{same:?}");
        assert_eq!(same.runner, "disk cache hit");
        assert_eq!(same.digest.len(), 64);
        let changed = by_key(&second, CHANGED);
        assert!(
            !changed.cache_hit && changed.remote_cacheable,
            "{changed:?}"
        );
        assert_eq!(changed.runner, "linux-sandbox");
        assert!(changed.total_millis > 0);
        assert_eq!(changed.args[0], "/bin/bash");
        assert!(changed.env.contains_key("PATH"));
        let nocache = by_key(&second, NOCACHE);
        assert!(
            !nocache.remote_cacheable && nocache.digest.is_empty(),
            "{nocache:?}"
        );
    }

    #[test]
    fn input_sets_resolve_to_file_digests() {
        let (first, second) = cache_logs();
        let old = first.inputs(by_key(&first, CHANGED)).unwrap();
        let new = second.inputs(by_key(&second, CHANGED)).unwrap();
        assert_eq!(
            new.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "changed.txt",
                "external/bazel_tools/tools/genrule/genrule-setup.sh"
            ]
        );
        assert_eq!(old["changed.txt"], OLD);
        assert_ne!(old["changed.txt"], new["changed.txt"]);
    }

    #[test]
    fn diff_names_the_changed_input_and_the_uncacheable_spawn() {
        let (first, second) = cache_logs();
        let new = second.inputs(by_key(&second, CHANGED)).unwrap();
        let lines = diff(&first, &second, 20).unwrap();
        let at = |key: &str| lines.iter().position(|l| l.contains(key));
        let changed = at(CHANGED).unwrap();
        assert!(lines[changed].ends_with("(linux-sandbox)"), "{lines:?}");
        assert_eq!(
            lines[changed + 1],
            format!("  input changed.txt: {OLD} -> {}", new["changed.txt"])
        );
        let nocache = at(NOCACHE).unwrap();
        assert_eq!(lines[nocache + 1], "  not remote cacheable");
        assert_eq!(at(SAME), None, "{lines:?}");
        assert_eq!(lines.len(), 4, "{lines:?}");
    }

    #[test]
    fn diff_keeps_only_the_slowest_spawns() {
        let (first, mut second) = cache_logs();
        for spawn in &mut second.spawns {
            spawn.total_millis = if spawn.label == "//:changed" {
                2500
            } else {
                100
            };
        }
        let lines = diff(&first, &second, 1).unwrap();
        assert_eq!(lines[0], format!("2.5s {CHANGED} (linux-sandbox)"));
        assert_eq!(lines.len(), 2, "{lines:?}");
    }

    fn spawn_with(args: &[&str], env: &[(&str, &str)], digest: &str) -> Spawn {
        Spawn {
            label: "//:x".to_owned(),
            mnemonic: "M".to_owned(),
            runner: "local".to_owned(),
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            remote_cacheable: true,
            digest: digest.to_owned(),
            ..Spawn::default()
        }
    }

    fn one(spawn: Spawn) -> ExecLog {
        ExecLog {
            spawns: vec![spawn],
            ..ExecLog::default()
        }
    }

    #[test]
    fn diff_names_changed_args_and_env() {
        let first = one(spawn_with(
            &["tool", "-O1"],
            &[("A", "1"), ("B", "2")],
            "d1",
        ));
        let second = one(spawn_with(
            &["tool", "-O2"],
            &[("A", "9"), ("C", "3")],
            "d2",
        ));
        assert_eq!(
            diff(&first, &second, 20).unwrap(),
            [
                "0.0s //:x M  (local)",
                "  arg 001: -O1 -> -O2",
                "  env A: 1 -> 9",
                "  env C: only in the second log",
                "  env B: only in the first log",
            ]
        );
    }

    #[test]
    fn diff_explains_a_spawn_that_never_reached_the_cache() {
        let first = one(spawn_with(&["tool"], &[], "d1"));
        let second = one(spawn_with(&["tool"], &[], "d1"));
        assert_eq!(
            diff(&first, &second, 20).unwrap()[1],
            "  same action as in the first log (local): its result never reached the cache"
        );
        let second = one(spawn_with(&["tool"], &[], "d2"));
        assert_eq!(
            diff(&first, &second, 20).unwrap()[1],
            "  digest differs, but inputs, args and env match"
        );
        let undigested = one(spawn_with(&["tool"], &[], ""));
        assert_eq!(
            diff(&undigested, &undigested, 20).unwrap()[1],
            "  same action as in the first log (local): its result never reached the cache"
        );
        assert_eq!(
            diff(&undigested, &one(spawn_with(&["tool", "-v"], &[], "")), 20).unwrap()[1],
            "  arg 001: only in the second log"
        );
        assert_eq!(
            diff(&ExecLog::default(), &second, 20).unwrap()[1],
            "  no spawn with this key in the first log"
        );
    }

    fn entry(id: u8, body: &[u8]) -> Vec<u8> {
        let entry = [vec![0x08, id], body.to_vec()].concat();
        [vec![u8::try_from(entry.len()).unwrap()], entry].concat()
    }

    #[test]
    fn runfiles_trees_resolve_to_their_files_and_repo_mapping() {
        let digest = |hash: &str| field(0x12, &field(0x0a, hash.as_bytes()));
        let file = [field(0x0a, b"tool.sh"), digest("aa")].concat();
        let tree = [
            field(0x0a, b"bin/t.runfiles"),
            vec![0x10, 0x02],
            field(0x32, &digest("bb")),
        ]
        .concat();
        let spawn = [
            vec![0x20, 0x04],
            field(0x3a, b"//:t"),
            field(0x42, b"TestRunner"),
        ]
        .concat();
        let raw = [
            entry(1, &field(0x1a, &file)),
            entry(2, &field(0x32, &field(0x2a, &[0x01]))),
            entry(3, &field(0x52, &tree)),
            entry(4, &field(0x32, &field(0x2a, &[0x03]))),
            entry(0, &field(0x3a, &spawn)),
        ]
        .concat();
        let zst = ruzstd::encoding::compress_to_vec(
            raw.as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        let log = ExecLog::parse(&zst).unwrap();
        let inputs = log.inputs(&log.spawns[0]).unwrap();
        assert_eq!(inputs["tool.sh"], "aa");
        assert_eq!(inputs["bin/t.runfiles/_repo_mapping"], "bb");
        assert_eq!(inputs.len(), 2, "{inputs:?}");
    }

    #[test]
    fn an_input_without_an_entry_is_an_error() {
        let log = one(Spawn {
            input_set: 7,
            ..Spawn::default()
        });
        let err = log.inputs(&log.spawns[0]).unwrap_err();
        assert!(err.contains("input 7 has no entry"), "{err}");
    }

    #[test]
    fn input_ids_read_packed_and_unpacked() {
        let mut found = Vec::new();
        ids(Value::Bytes(&[0x07, 0x02]), &mut found).unwrap();
        ids(Value::Varint(9), &mut found).unwrap();
        assert_eq!(found, [7, 2, 9]);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(read(b"not zstd").is_err());
    }
}
