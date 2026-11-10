//! Reader for Bazel's `--execution_log_compact_file` and a comparison of two logs.
//!
//! The log is a zstd stream of varint-length `ExecLogEntry` messages. Every spawn output
//! becomes the key `<label> <mnemonic> <output path>`, valued with the file digest, the sorted
//! child digests of a directory, or the target of an unresolved symlink.
//!
//! The fixtures in `exec_log_fixtures` were recorded with Bazel 9.3.1 from a scratch workspace
//! holding `.bazelversion` (`9.3.1`), an empty `MODULE.bazel` and a `BUILD.bazel` with two
//! genrules: `stable` (`echo hi > $@`) and `stamped` (`date +%s%N > $@`):
//!
//! ```text
//! bazel --output_base=$PWD/ob_first --bazelrc=/dev/null build --noenable_workspace \
//!   --remote_cache= --remote_executor= --disk_cache= \
//!   --execution_log_compact_file=$PWD/first.log.zst //:stable //:stamped
//! bazel --output_base=$PWD/ob_second --bazelrc=/dev/null build --noenable_workspace \
//!   --remote_cache= --remote_executor= --disk_cache= \
//!   --execution_log_compact_file=$PWD/second.log.zst //:stable //:stamped
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;

pub type Log = BTreeMap<String, String>;

enum Node {
    File { path: String, hash: String },
    Directory { path: String, hashes: Vec<String> },
    Symlink { path: String, target: String },
}

struct Spawn {
    outputs: Vec<u64>,
    label: String,
    mnemonic: String,
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
    let (mut path, mut hashes) = (String::new(), Vec::new());
    for field in Fields::new(bytes) {
        match field? {
            (1, Value::Bytes(value)) => path = text(value)?,
            (2, Value::Bytes(value)) => hashes.push(file(value)?.1),
            _ => {}
        }
    }
    hashes.sort();
    Ok(Node::Directory { path, hashes })
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

fn spawn(bytes: &[u8]) -> Result<Spawn, String> {
    let mut found = Spawn {
        outputs: Vec::new(),
        label: String::new(),
        mnemonic: String::new(),
    };
    for field in Fields::new(bytes) {
        match field? {
            (6, Value::Bytes(output)) => {
                for inner in Fields::new(output) {
                    if let (5, Value::Varint(id)) = inner? {
                        found.outputs.push(id);
                    }
                }
            }
            (7, Value::Bytes(value)) => found.label = text(value)?,
            (8, Value::Bytes(value)) => found.mnemonic = text(value)?,
            _ => {}
        }
    }
    Ok(found)
}

pub fn read(bytes: &[u8]) -> Result<Log, String> {
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(bytes).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    decoder.read_to_end(&mut raw).map_err(|e| e.to_string())?;

    let mut nodes = BTreeMap::new();
    let mut spawns = Vec::new();
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
                (7, Value::Bytes(value)) => spawns.push(spawn(value)?),
                _ => {}
            }
        }
        if let Some(node) = node {
            nodes.insert(id, node);
        }
    }

    let mut log = Log::new();
    for spawn in spawns {
        for output in spawn.outputs {
            let (path, value) = match nodes.get(&output) {
                Some(Node::File { path, hash }) => (path, hash.clone()),
                Some(Node::Directory { path, hashes }) => (path, hashes.join(" ")),
                Some(Node::Symlink { path, target }) => (path, target.clone()),
                None => return Err(format!("spawn output {output} has no entry")),
            };
            log.insert(format!("{} {} {path}", spawn.label, spawn.mnemonic), value);
        }
    }
    Ok(log)
}

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

    #[test]
    fn garbage_is_an_error() {
        assert!(read(b"not zstd").is_err());
    }
}
