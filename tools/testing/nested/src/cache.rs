//! The repository cache of a nested Bazel: built from declared downloads, and listed from a consumer lock.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The variable naming the runfiles keys of the `nested_repository_cache` manifests, space separated.
pub const MANIFESTS_ENV: &str = "DX_NESTED_REPOSITORY_CACHE";

fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Parses a manifest: one `<sha256> <runfiles key>` per line.
pub fn parse_manifest(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut entries = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match line.split_once(' ') {
            Some((sha256, file)) if is_sha256(sha256) && !file.trim().is_empty() => {
                entries.push((sha256.to_owned(), file.trim().to_owned()));
            }
            _ => return Err(format!("repository cache manifest: bad line {line:?}")),
        }
    }
    Ok(entries)
}

/// The place Bazel looks up a download with `sha256` in the repository cache at `cache`.
pub fn entry(cache: &Path, sha256: &str) -> PathBuf {
    cache
        .join("content_addressable")
        .join("sha256")
        .join(sha256)
        .join("file")
}

#[cfg(unix)]
fn place(src: &Path, dst: &Path) -> std::io::Result<()> {
    let src = std::path::absolute(src)?;
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(not(unix))]
fn place(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst).map(|_| ())
}

/// Builds the repository cache at `cache` from `files`, each a sha256 and the file that has it.
pub fn install(cache: &Path, files: &[(String, PathBuf)]) -> Result<(), String> {
    std::fs::create_dir_all(cache)
        .map_err(|error| format!("cannot create {}: {error}", cache.display()))?;
    for (sha256, src) in files {
        if !is_sha256(sha256) {
            return Err(format!("repository cache: bad sha256 {sha256:?}"));
        }
        let dst = entry(cache, sha256);
        if std::fs::symlink_metadata(&dst).is_ok() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        place(src, &dst).map_err(|error| {
            format!(
                "cannot put {} at {}: {error}",
                src.display(),
                dst.display()
            )
        })?;
    }
    Ok(())
}

fn base64_value(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Turns a `sha256-<base64>` integrity into the hex sha256.
pub fn integrity_hex(integrity: &str) -> Option<String> {
    let encoded = integrity.strip_prefix("sha256-")?.trim_end_matches('=');
    let mut bits = 0u32;
    let mut count = 0u32;
    let mut hex = String::new();
    for byte in encoded.bytes() {
        bits = (bits << 6) | base64_value(byte)?;
        count += 6;
        if count >= 8 {
            count -= 8;
            hex.push_str(&format!("{:02x}", (bits >> count) & 0xff));
        }
    }
    is_sha256(&hex).then_some(hex)
}

fn quoted_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = &text[text.find(key)? + key.len()..];
    let rest = &rest[rest.find('"')? + 1..];
    Some(&rest[..rest.find('"')?])
}

/// The `(url, sha256)` of every `archive_override` in a MODULE.bazel text.
pub fn archive_overrides(module: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for block in module.split("archive_override(").skip(1) {
        let block = block.split("\n)").next().unwrap_or(block);
        let sha256 = quoted_after(block, "integrity").and_then(integrity_hex);
        let url = quoted_after(block, "urls");
        if let (Some(url), Some(sha256)) = (url, sha256) {
            found.push((url.to_owned(), sha256));
        }
    }
    found
}

/// Renders the cache list of a consumer: every registry file of `lock`, plus every module archive,
/// patch, overlay file and `archive_override` archive that `fetched` holds.
///
/// `fetched` returns the bytes of a download by sha256, read from the repository cache of an online run.
pub fn list(
    lock: &str,
    module: &str,
    fetched: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<String, String> {
    let lock: serde_json::Value =
        serde_json::from_str(lock).map_err(|error| format!("MODULE.bazel.lock: {error}"))?;
    let registry = lock["registryFileHashes"]
        .as_object()
        .ok_or("MODULE.bazel.lock: no registryFileHashes")?;
    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    let mut add = |url: &str, sha256: &str| {
        entries.insert(url.to_owned(), sha256.to_owned());
    };
    for (url, sha256) in registry {
        let Some(sha256) = sha256.as_str().filter(|sha256| is_sha256(sha256)) else {
            continue;
        };
        let bytes = fetched(sha256).ok_or_else(|| format!("{url} was not downloaded"))?;
        add(url, sha256);
        if !url.ends_with("/source.json") {
            continue;
        }
        let source: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|error| format!("{url}: {error}"))?;
        let archive = source["url"].as_str().zip(source["integrity"].as_str());
        if let Some((archive, integrity)) = archive {
            let sha256 = integrity_hex(integrity).ok_or_else(|| format!("{url}: bad integrity"))?;
            if fetched(&sha256).is_some() {
                add(archive, &sha256);
            }
        }
        let base = url.trim_end_matches("source.json");
        for kind in ["patches", "overlay"] {
            let Some(files) = source[kind].as_object() else {
                continue;
            };
            for (name, integrity) in files {
                let sha256 = integrity
                    .as_str()
                    .and_then(integrity_hex)
                    .ok_or_else(|| format!("{url}: bad integrity of {name}"))?;
                if fetched(&sha256).is_some() {
                    add(&format!("{base}{kind}/{name}"), &sha256);
                }
            }
        }
    }
    for (url, sha256) in archive_overrides(module) {
        if fetched(&sha256).is_some() {
            add(&url, &sha256);
        }
    }
    Ok(entries
        .iter()
        .map(|(url, sha256)| format!("{url} {sha256}\n"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "4e50948fbb8564a8b3a3b1f354cd9ec7ffdfe304f63c6a5feb1fdbcfbfc782bd";
    const A_INTEGRITY: &str = "sha256-TlCUj7uFZKizo7HzVM2ex//f4wT2PGpf6x/bz7/Hgr0=";

    fn sha(digit: char) -> String {
        std::iter::repeat_n(digit, 64).collect()
    }

    #[test]
    fn manifest_lines_parse_and_bad_lines_are_named() {
        let text = format!("{A} nested_cache_x/file/file\n\n");
        assert_eq!(
            parse_manifest(&text),
            Ok(vec![(A.to_owned(), "nested_cache_x/file/file".to_owned())])
        );
        assert_eq!(
            parse_manifest("abc file\n"),
            Err("repository cache manifest: bad line \"abc file\"".to_owned())
        );
        assert!(parse_manifest(&format!("{A}\n")).is_err());
    }

    #[test]
    fn install_puts_each_file_where_bazel_looks_it_up() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("download");
        std::fs::write(&src, "bytes").expect("write");
        let cache = dir.path().join("cache");
        let files = vec![(A.to_owned(), src)];
        install(&cache, &files).expect("install");
        install(&cache, &files).expect("install again");
        let placed = cache.join(format!("content_addressable/sha256/{A}/file"));
        assert_eq!(entry(&cache, A), placed);
        assert_eq!(std::fs::read_to_string(placed).expect("read"), "bytes");
    }

    #[test]
    fn install_rejects_a_bad_sha256() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = vec![("../x".to_owned(), dir.path().join("download"))];
        assert_eq!(
            install(&dir.path().join("cache"), &files),
            Err("repository cache: bad sha256 \"../x\"".to_owned())
        );
    }

    #[test]
    fn integrity_turns_into_hex() {
        assert_eq!(integrity_hex(A_INTEGRITY).as_deref(), Some(A));
        assert_eq!(integrity_hex("sha512-AAAA"), None);
        assert_eq!(integrity_hex("sha256-AAAA"), None);
    }

    #[test]
    fn archive_overrides_pair_the_first_url_with_the_integrity() {
        let module = format!(
            "bazel_dep(name = \"a\")\narchive_override(\n    module_name = \"a\",\n    integrity = \"{A_INTEGRITY}\",\n    urls = [\"https://example.com/a.tar.gz\"],\n)\n"
        );
        assert_eq!(
            archive_overrides(&module),
            vec![("https://example.com/a.tar.gz".to_owned(), A.to_owned())]
        );
        assert!(archive_overrides("module(name = \"m\")\n").is_empty());
    }

    #[test]
    fn list_names_registry_files_and_only_the_archives_that_were_fetched() {
        let (module_sha, source_sha, unfetched_source_sha, patch_sha) =
            (sha('1'), sha('2'), sha('3'), sha('4'));
        let lock = serde_json::json!({"registryFileHashes": {
            "https://r/modules/a/1/MODULE.bazel": module_sha,
            "https://r/modules/a/1/source.json": source_sha,
            "https://r/modules/b/1/source.json": unfetched_source_sha,
            "https://r/modules/c/1/MODULE.bazel": "not found",
        }})
        .to_string();
        let patch_integrity = format!("sha256-{}REQ=", "RERE".repeat(10));
        assert_eq!(integrity_hex(&patch_integrity), Some(patch_sha.clone()));
        let source = serde_json::json!({
            "url": "https://example.com/a.tar.gz",
            "integrity": A_INTEGRITY,
            "patches": {"fix.patch": patch_integrity},
        })
        .to_string();
        let other = serde_json::json!({
            "url": "https://example.com/b.tar.gz",
            "integrity": format!("sha256-{}U=", "V".repeat(42)),
        })
        .to_string();
        let fetched = |sha256: &str| -> Option<Vec<u8>> {
            if sha256 == source_sha {
                Some(source.clone().into_bytes())
            } else if sha256 == unfetched_source_sha {
                Some(other.clone().into_bytes())
            } else if sha256 == module_sha || sha256 == patch_sha || sha256 == A {
                Some(Vec::new())
            } else {
                None
            }
        };
        assert_eq!(
            list(&lock, "", &fetched),
            Ok(format!(
                "https://example.com/a.tar.gz {A}\nhttps://r/modules/a/1/MODULE.bazel {module_sha}\nhttps://r/modules/a/1/patches/fix.patch {patch_sha}\nhttps://r/modules/a/1/source.json {source_sha}\nhttps://r/modules/b/1/source.json {unfetched_source_sha}\n"
            ))
        );
    }

    #[test]
    fn list_names_a_registry_file_that_was_not_downloaded() {
        let lock = serde_json::json!({"registryFileHashes": {"https://r/modules/a/1/MODULE.bazel": sha('1')}})
            .to_string();
        assert_eq!(
            list(&lock, "", &|_| None),
            Err("https://r/modules/a/1/MODULE.bazel was not downloaded".to_owned())
        );
    }
}
