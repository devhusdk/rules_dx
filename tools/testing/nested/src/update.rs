//! The update run of a nested test: records every download and rewrites the cache list and the locks.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The source tree an update run rewrites.
#[derive(Clone, Debug)]
pub struct Target {
    /// The checkout `bazel run` started in.
    pub workspace: PathBuf,
    /// The cache list, relative to `workspace`.
    pub list: String,
}

const LOGS: &str = "rules-log";
const LOCKS: &str = "locks";
const RC: &str = "update.rc";

fn io<T>(result: std::io::Result<T>, what: &str, path: &Path) -> Result<T, String> {
    result.map_err(|error| format!("cannot {what} {}: {error}", path.display()))
}

/// Prepares `home` for recording and returns the rc file the shim at `wrapper` rewrites per command.
pub fn start(home: &Path, wrapper: &Path) -> Result<PathBuf, String> {
    for dir in [LOGS, LOCKS] {
        io(
            std::fs::create_dir_all(home.join(dir)),
            "create",
            &home.join(dir),
        )?;
    }
    let rc = home.join(RC);
    io(std::fs::write(&rc, ""), "write", &rc)?;
    let sidecar = wrapper.with_extension("update");
    if let Some(bin) = sidecar.parent() {
        io(std::fs::create_dir_all(bin), "create", bin)?;
    }
    io(
        std::fs::write(&sidecar, home.to_string_lossy().as_bytes()),
        "write",
        &sidecar,
    )?;
    Ok(rc)
}

/// Points the next Bazel command of the shim `me` at a fresh rules log; a no-op outside an update run.
pub fn before_command(me: &Path) -> Result<(), String> {
    let Ok(home) = std::fs::read_to_string(me.with_extension("update")) else {
        return Ok(());
    };
    let home = PathBuf::from(home);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let log = home
        .join(LOGS)
        .join(format!("{stamp}-{}.pb", std::process::id()));
    let rc = home.join(RC);
    let line = format!(
        "common --experimental_workspace_rules_log_file={}\n",
        log.to_string_lossy().replace('\\', "/")
    );
    io(std::fs::write(&rc, line), "write", &rc)
}

/// Writes the lock Bazel left in `root` to `rel` in the source tree and keeps a copy for [`finish`].
pub fn keep_lock(home: &Path, target: &Target, root: &Path, rel: &str) -> Result<(), String> {
    let lock = root.join("MODULE.bazel.lock");
    let text = io(std::fs::read_to_string(&lock), "read", &lock)?;
    let kept = home.join(LOCKS).join(rel.replace(['/', '\\'], "_"));
    io(std::fs::write(&kept, &text), "write", &kept)?;
    let dest = target.workspace.join(rel);
    io(std::fs::write(&dest, &text), "write", &dest)
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Decodes a `sha256-<base64>` integrity value to lowercase hex.
fn integrity_hex(text: &str) -> Option<String> {
    let encoded = text.strip_prefix("sha256-")?.trim_end_matches('=');
    let mut bits = 0u32;
    let mut count = 0u32;
    let mut hex = String::new();
    for byte in encoded.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            hex.push_str(&format!("{:02x}", (bits >> count) & 0xff));
        }
    }
    is_sha256(&hex).then_some(hex)
}

fn varint(bytes: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *bytes.get(*pos)?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

/// The length-delimited fields of one protobuf message; `None` when `bytes` is not a message.
fn delimited(bytes: &[u8]) -> Option<Vec<&[u8]>> {
    let mut pos = 0;
    let mut fields = Vec::new();
    while pos < bytes.len() {
        let key = varint(bytes, &mut pos)?;
        match key & 7 {
            0 => {
                varint(bytes, &mut pos)?;
            }
            1 => pos = pos.checked_add(8)?,
            5 => pos = pos.checked_add(4)?,
            2 => {
                let len = usize::try_from(varint(bytes, &mut pos)?).ok()?;
                let end = pos.checked_add(len)?;
                fields.push(bytes.get(pos..end)?);
                pos = end;
            }
            _ => return None,
        }
        if pos > bytes.len() {
            return None;
        }
    }
    Some(fields)
}

/// Every checksummed download in a Bazel workspace rules log, as `(urls, sha256)`.
pub fn downloads(log: &[u8]) -> Vec<(Vec<String>, String)> {
    let mut found = Vec::new();
    let mut pos = 0;
    while pos < log.len() {
        let Some(len) = varint(log, &mut pos).and_then(|len| usize::try_from(len).ok()) else {
            break;
        };
        let Some(event) = pos.checked_add(len).and_then(|end| log.get(pos..end)) else {
            break;
        };
        pos += len;
        for field in delimited(event).unwrap_or_default() {
            let strings: Vec<&str> = delimited(field)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|bytes| std::str::from_utf8(bytes).ok())
                .collect();
            let urls: Vec<String> = strings
                .iter()
                .filter(|text| text.starts_with("https://") || text.starts_with("http://"))
                .map(|text| (*text).to_owned())
                .collect();
            let sha256 = strings.iter().find_map(|text| {
                if is_sha256(text) {
                    Some((*text).to_owned())
                } else {
                    integrity_hex(text)
                }
            });
            if let (false, Some(sha256)) = (urls.is_empty(), sha256) {
                found.push((urls, sha256));
            }
        }
    }
    found
}

fn files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths: Vec<PathBuf> = io(std::fs::read_dir(dir), "list", dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    Ok(paths)
}

/// Rewrites the cache list from what the run left in the repository cache under `home`.
///
/// Lists each cached file that a lock or a rules log gives a URL for, and returns their number.
pub fn finish(home: &Path, target: &Target) -> Result<usize, String> {
    let mut urls: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for lock in files(&home.join(LOCKS))? {
        let text = io(std::fs::read_to_string(&lock), "read", &lock)?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", lock.display()))?;
        let hashes = json["registryFileHashes"].as_object();
        for (url, sha256) in hashes.into_iter().flatten() {
            if let Some(sha256) = sha256.as_str().filter(|text| is_sha256(text)) {
                urls.insert(sha256.to_owned(), vec![url.clone()]);
            }
        }
    }
    for log in files(&home.join(LOGS))? {
        let bytes = io(std::fs::read(&log), "read", &log)?;
        for (found, sha256) in downloads(&bytes) {
            urls.entry(sha256).or_insert(found);
        }
    }
    let cache = home
        .join(super::REPOSITORY_CACHE)
        .join("content_addressable/sha256");
    let cached: BTreeSet<String> = if cache.is_dir() {
        files(&cache)?
            .iter()
            .filter_map(|path| path.file_name()?.to_str().map(str::to_owned))
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut text = String::new();
    let mut count = 0;
    for sha256 in &cached {
        if let Some(found) = urls.get(sha256) {
            text.push_str(&format!("{sha256} {}\n", found.join(" ")));
            count += 1;
        }
    }
    let list = target.workspace.join(&target.list);
    io(std::fs::write(&list, text), "write", &list)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "4e50948fbb8564a8b3a3b1f354cd9ec7ffdfe304f63c6a5feb1fdbcfbfc782bd";

    fn length(mut len: usize) -> Vec<u8> {
        let mut out = Vec::new();
        while len >= 0x80 {
            out.push(u8::try_from(len & 0x7f).expect("7 bits") | 0x80);
            len >>= 7;
        }
        out.push(u8::try_from(len).expect("7 bits"));
        out
    }

    fn field(number: u8, bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![(number << 3) | 2];
        out.extend(length(bytes.len()));
        out.extend_from_slice(bytes);
        out
    }

    fn event(download: &[u8]) -> Vec<u8> {
        let mut body = field(1, b"/x/MODULE.bazel:3:5");
        body.extend(field(5, download));
        body.extend([0x20, 0x01]);
        let mut out = length(body.len());
        out.extend(body);
        out
    }

    #[test]
    fn integrity_decodes_to_hex() {
        assert_eq!(
            integrity_hex("sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=").as_deref(),
            Some(SHA)
        );
        assert_eq!(integrity_hex("sha256-AAAA"), None);
        assert_eq!(integrity_hex(SHA), None);
    }

    #[test]
    fn downloads_read_urls_with_a_sha256_or_an_integrity() {
        let mut hex = field(1, b"https://a.example/x.tgz");
        hex.extend(field(1, b"https://b.example/x.tgz"));
        hex.extend(field(2, b"out"));
        hex.extend(field(3, SHA.as_bytes()));
        let mut sri = field(1, b"https://c.example/y.tgz");
        sri.extend(field(
            6,
            b"sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=",
        ));
        let unchecked = field(1, b"https://d.example/SHA256SUMS");
        let mut log = event(&hex);
        log.extend(event(&sri));
        log.extend(event(&unchecked));
        assert_eq!(
            downloads(&log),
            vec![
                (
                    vec![
                        "https://a.example/x.tgz".to_owned(),
                        "https://b.example/x.tgz".to_owned()
                    ],
                    SHA.to_owned()
                ),
                (vec!["https://c.example/y.tgz".to_owned()], SHA.to_owned()),
            ]
        );
        assert!(downloads(&[0xff, 0xff]).is_empty());
    }

    #[test]
    fn finish_lists_every_cached_file_with_a_recorded_url() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path().join("home");
        let wrapper = home.join("bin/bazel");
        std::fs::create_dir_all(home.join("bin")).expect("bin");
        let rc = start(&home, &wrapper).expect("start");
        before_command(&wrapper).expect("before");
        let line = std::fs::read_to_string(&rc).expect("rc");
        let log = line
            .trim()
            .strip_prefix("common --experimental_workspace_rules_log_file=")
            .expect("log flag");
        let mut download = field(1, b"https://a.example/x.tgz");
        download.extend(field(3, SHA.as_bytes()));
        std::fs::write(log, event(&download)).expect("log");
        before_command(&dir.path().join("other")).expect("no sidecar");

        let registry = "1".repeat(64);
        let root = dir.path().join("ws");
        let target = Target {
            workspace: dir.path().join("src"),
            list: "pkg/list.txt".to_owned(),
        };
        std::fs::create_dir_all(&root).expect("ws");
        std::fs::create_dir_all(target.workspace.join("pkg")).expect("src");
        let lock = format!(
            "{{\"registryFileHashes\": {{\"https://r.example/m/MODULE.bazel\": \"{registry}\", \"https://r.example/n\": \"not found\"}}}}"
        );
        std::fs::write(root.join("MODULE.bazel.lock"), &lock).expect("lock");
        keep_lock(&home, &target, &root, "pkg/MODULE.bazel.lock").expect("keep");
        assert_eq!(
            std::fs::read_to_string(target.workspace.join("pkg/MODULE.bazel.lock")).expect("kept"),
            lock
        );

        let cache = home
            .join(crate::REPOSITORY_CACHE)
            .join("content_addressable/sha256");
        for sha256 in [SHA, registry.as_str()] {
            std::fs::create_dir_all(cache.join(sha256)).expect("entry");
        }
        assert_eq!(finish(&home, &target), Ok(2));
        assert_eq!(
            std::fs::read_to_string(target.workspace.join("pkg/list.txt")).expect("list"),
            format!("{registry} https://r.example/m/MODULE.bazel\n{SHA} https://a.example/x.tgz\n")
        );

        let stray = "2".repeat(64);
        std::fs::create_dir_all(cache.join(&stray)).expect("stray");
        assert_eq!(finish(&home, &target), Ok(2));
    }
}
