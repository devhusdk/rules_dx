//! The repository cache of a nested Bazel: built from declared downloads, and listed from a consumer lock.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The variable naming the runfiles keys of the `nested_repository_cache` manifests, space separated.
pub const MANIFESTS_ENV: &str = "DX_NESTED_REPOSITORY_CACHE";

/// The checksum algorithm of a lowercase hex digest, named as in the repository cache.
fn algorithm(digest: &str) -> Option<&'static str> {
    if !digest
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    match digest.len() {
        64 => Some("sha256"),
        96 => Some("sha384"),
        128 => Some("sha512"),
        _ => None,
    }
}

/// One download of a cache set: its hex digest, the file that has it, and the URLs it is fetched from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download<F> {
    pub digest: String,
    pub file: F,
    pub urls: Vec<String>,
}

/// Parses a manifest: one `<hex digest> <runfiles key> <url>...` per line.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn parse_manifest(text: &str) -> Result<Vec<Download<String>>, String> {
    let mut entries = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next()) {
            (Some(digest), Some(file)) if algorithm(digest).is_some() => {
                entries.push(Download {
                    digest: digest.to_owned(),
                    file: file.to_owned(),
                    urls: fields.map(str::to_owned).collect(),
                });
            }
            _ => return Err(format!("repository cache manifest: bad line {line:?}")),
        }
    }
    Ok(entries)
}

/// The place Bazel looks up a download with the hex `digest` in the repository cache at `cache`.
#[must_use]
pub fn entry(cache: &Path, digest: &str) -> PathBuf {
    cache
        .join("content_addressable")
        .join(algorithm(digest).unwrap_or("sha256"))
        .join(digest)
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

/// The hex digest, in the algorithm of `digest`, of a canonical id.
fn id_hash(digest: &str, canonical_id: &str) -> Option<String> {
    let bytes = canonical_id.as_bytes();
    match algorithm(digest)? {
        "sha256" => Some(dx_digest::sha256_hex(bytes)),
        "sha384" => Some(dx_digest::sha384_hex(bytes)),
        _ => Some(dx_digest::sha512_hex(bytes)),
    }
}

/// The prefix of a canonical id hash file in a repository cache entry and of its cache list token.
pub const ID_PREFIX: &str = "id-";

/// The `id-<hex>` canonical id hashes in the cache entry of `digest` that no URL in `urls` accounts for.
#[must_use]
pub fn extra_ids(cache: &Path, digest: &str, urls: &[&str]) -> Vec<String> {
    let known: Vec<String> = urls.iter().filter_map(|url| id_hash(digest, url)).collect();
    let Some(dir) = entry(cache, digest).parent().map(Path::to_path_buf) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter(|name| {
                    name.strip_prefix(ID_PREFIX).is_some_and(|hash| {
                        algorithm(hash) == algorithm(digest) && !known.iter().any(|k| k == hash)
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// Builds the repository cache at `cache` from `files`; each URL is also recorded as a canonical id
/// and each `id-<hex>` token is recorded as that canonical id hash.
///
/// # Errors
///
/// Returns `Err` when the operation fails.
pub fn install(cache: &Path, files: &[Download<PathBuf>]) -> Result<(), String> {
    std::fs::create_dir_all(cache)
        .map_err(|error| format!("cannot create {}: {error}", cache.display()))?;
    for Download { digest, file, urls } in files {
        if algorithm(digest).is_none() {
            return Err(format!("repository cache: bad digest {digest:?}"));
        }
        let dst = entry(cache, digest);
        let dir = dst.parent().unwrap_or(cache);
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
        if std::fs::symlink_metadata(&dst).is_err() {
            place(file, &dst).map_err(|error| {
                format!(
                    "cannot put {} at {}: {error}",
                    file.display(),
                    dst.display()
                )
            })?;
        }
        for url in urls {
            let hash = match url.strip_prefix(ID_PREFIX) {
                Some(hash) if algorithm(hash) == algorithm(digest) => hash.to_owned(),
                Some(_) => return Err(format!("repository cache: bad canonical id hash {url:?}")),
                None => match id_hash(digest, url) {
                    Some(hash) => hash,
                    None => continue,
                },
            };
            let id = dir.join(format!("{ID_PREFIX}{hash}"));
            std::fs::write(&id, "")
                .map_err(|error| format!("cannot write {}: {error}", id.display()))?;
        }
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

/// Turns a `sha256-`, `sha384-` or `sha512-<base64>` integrity into its hex digest.
#[must_use]
pub fn integrity_hex(integrity: &str) -> Option<String> {
    let (name, encoded) = integrity.split_once('-')?;
    let encoded = encoded.trim_end_matches('=');
    let mut bits = 0u32;
    let mut count = 0u32;
    let mut hex = String::new();
    for byte in encoded.bytes() {
        bits = (bits << 6) | base64_value(byte)?;
        count += 6;
        if count >= 8 {
            count -= 8;
            write!(hex, "{:02x}", (bits >> count) & 0xff).ok();
        }
    }
    (algorithm(&hex) == Some(name)).then_some(hex)
}

fn quoted_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let (_, rest) = text.split_once(key)?;
    let (_, rest) = rest.split_once('"')?;
    let (value, _) = rest.split_once('"')?;
    Some(value)
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
///
/// # Errors
///
/// Returns `Err` when the operation fails.
/// `fetched` returns the bytes of a download by sha256, read from the repository cache of an online run.
pub fn list(
    lock: &str,
    module: &str,
    fetched: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<String, String> {
    let lock: serde_json::Value =
        serde_json::from_str(lock).map_err(|error| format!("MODULE.bazel.lock: {error}"))?;
    let registry = lock
        .get("registryFileHashes")
        .and_then(serde_json::Value::as_object)
        .ok_or("MODULE.bazel.lock: no registryFileHashes")?;
    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    let mut add = |url: &str, sha256: &str| {
        entries.insert(url.to_owned(), sha256.to_owned());
    };
    for (url, sha256) in registry {
        let Some(sha256) = sha256
            .as_str()
            .filter(|sha256| algorithm(sha256) == Some("sha256"))
        else {
            continue;
        };
        let bytes = fetched(sha256).ok_or_else(|| format!("{url} was not downloaded"))?;
        add(url, sha256);
        if !url.ends_with("/source.json") {
            continue;
        }
        let source: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|error| format!("{url}: {error}"))?;
        let field = |name: &str| source.get(name).and_then(serde_json::Value::as_str);
        let archive = field("url").zip(field("integrity"));
        if let Some((archive, integrity)) = archive {
            let sha256 = integrity_hex(integrity).ok_or_else(|| format!("{url}: bad integrity"))?;
            if fetched(&sha256).is_some() {
                add(archive, &sha256);
            }
        }
        let base = url.trim_end_matches("source.json");
        for kind in ["patches", "overlay"] {
            let Some(files) = source.get(kind).and_then(serde_json::Value::as_object) else {
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
        .fold(String::new(), |mut list, (url, sha256)| {
            writeln!(list, "{url} {sha256}").ok();
            list
        }))
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
            Ok(vec![Download {
                digest: A.to_owned(),
                file: "nested_cache_x/file/file".to_owned(),
                urls: Vec::new(),
            }])
        );
        assert_eq!(
            parse_manifest("abc file\n"),
            Err("repository cache manifest: bad line \"abc file\"".to_owned())
        );
        assert!(parse_manifest(&format!("{A}\n")).is_err());
        assert_eq!(
            parse_manifest(&format!("{A} f https://a/x https://b/x\n")),
            Ok(vec![Download {
                digest: A.to_owned(),
                file: "f".to_owned(),
                urls: vec!["https://a/x".to_owned(), "https://b/x".to_owned()],
            }])
        );
    }

    #[test]
    fn an_id_token_is_recorded_as_that_canonical_id_hash_and_found_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("download");
        std::fs::write(&src, "bytes").expect("write");
        let cache = dir.path().join("cache");
        let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let other = "bbea0fabfdbf037ddf6765848ce5875c610f16b7cd81c9c19c67665f0a3aca54";
        let files = vec![Download {
            digest: A.to_owned(),
            file: src,
            urls: vec!["abc".to_owned(), format!("{ID_PREFIX}{other}")],
        }];
        install(&cache, &files).expect("install");
        let placed = entry(&cache, A);
        assert!(placed.with_file_name(format!("id-{hash}")).is_file());
        assert!(placed.with_file_name(format!("id-{other}")).is_file());
        assert_eq!(extra_ids(&cache, A, &["abc"]), vec![format!("id-{other}")]);
        assert!(extra_ids(&cache, &sha('b'), &["abc"]).is_empty());
    }

    #[test]
    fn install_rejects_an_id_token_of_another_algorithm() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("download");
        std::fs::write(&src, "bytes").expect("write");
        let files = vec![Download {
            digest: A.to_owned(),
            file: src,
            urls: vec!["id-abc".to_owned()],
        }];
        assert_eq!(
            install(&dir.path().join("cache"), &files),
            Err("repository cache: bad canonical id hash \"id-abc\"".to_owned())
        );
    }

    #[test]
    fn install_puts_each_file_where_bazel_looks_it_up() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("download");
        std::fs::write(&src, "bytes").expect("write");
        let cache = dir.path().join("cache");
        let files = vec![Download {
            digest: A.to_owned(),
            file: src,
            urls: vec!["abc".to_owned()],
        }];
        install(&cache, &files).expect("install");
        install(&cache, &files).expect("install again");
        let placed = cache.join(format!("content_addressable/sha256/{A}/file"));
        assert_eq!(entry(&cache, A), placed);
        assert_eq!(std::fs::read_to_string(&placed).expect("read"), "bytes");
        let id = "id-ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(
            placed.with_file_name(id).is_file(),
            "canonical id of the URL"
        );
    }

    #[test]
    fn a_sha512_download_records_its_url_as_a_sha512_canonical_id() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = dir.path().join("download");
        std::fs::write(&src, "bytes").expect("write");
        let cache = dir.path().join("cache");
        let digest = "d".repeat(128);
        let files = vec![Download {
            digest: digest.clone(),
            file: src,
            urls: vec!["abc".to_owned()],
        }];
        install(&cache, &files).expect("install");
        let id = entry(&cache, &digest).with_file_name(
            "id-ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        );
        assert!(id.is_file(), "{}", id.display());
    }

    #[test]
    fn install_rejects_a_bad_digest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = vec![Download {
            digest: "../x".to_owned(),
            file: dir.path().join("download"),
            urls: Vec::new(),
        }];
        assert_eq!(
            install(&dir.path().join("cache"), &files),
            Err("repository cache: bad digest \"../x\"".to_owned())
        );
    }

    #[test]
    fn a_sha384_digest_goes_under_sha384() {
        let digest = "c".repeat(96);
        assert_eq!(
            entry(Path::new("/c"), &digest),
            Path::new("/c/content_addressable/sha384")
                .join(&digest)
                .join("file")
        );
        assert_eq!(
            parse_manifest(&format!("{digest} f\n")),
            Ok(vec![Download {
                digest: digest.clone(),
                file: "f".to_owned(),
                urls: Vec::new(),
            }])
        );
    }

    #[test]
    fn integrity_turns_into_hex() {
        assert_eq!(integrity_hex(A_INTEGRITY).as_deref(), Some(A));
        assert_eq!(integrity_hex("sha512-AAAA"), None);
        assert_eq!(integrity_hex("sha256-AAAA"), None);
        assert_eq!(
            integrity_hex(
                "sha384-jEOdMXHPvRiaFXWRnSA5Mw41VCKkNPv42i35i9BiVxrAnfyl/3/Li/iyQyj16QIr"
            )
            .as_deref(),
            Some(
                "8c439d3171cfbd189a1575919d2039330e355422a434fbf8da2df98bd062571ac09dfca5ff7fcb8bf8b24328f5e9022b"
            )
        );
        assert_eq!(
            integrity_hex(
                "sha256-jEOdMXHPvRiaFXWRnSA5Mw41VCKkNPv42i35i9BiVxrAnfyl/3/Li/iyQyj16QIr"
            ),
            None
        );
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
        assert_eq!(
            archive_overrides("module(name = \"m\")\n"),
            [] as [(String, String); 0]
        );
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
