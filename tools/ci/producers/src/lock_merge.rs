//! Merges the per-platform `MODULE.bazel.lock` files of `lock-refresh.yml` into one lock.
//! Each platform lock contributes only the module extension entries keyed by its own factor.
//!
//! The merged lock must come out byte for byte the way Bazel writes it. Bazel writes its
//! records (the lock root, each extension entry) in field order, not key order, and it only
//! rewrites the lock when the content changes. A key-sorted lock is the same content, so the
//! workflow's normalise step leaves it alone and the pushed commit rewrites every line of the
//! lock. That is why this file keeps its own small JSON tree that remembers key order instead
//! of `serde_json::Value`, which sorts keys.

use std::path::Path;

const PLATFORMS: [(&str, &str); 5] = [
    ("linux_x86_64", "os:linux,arch:amd64"),
    ("linux_arm64", "os:linux,arch:aarch64"),
    ("macos_arm64", "os:osx,arch:aarch64"),
    ("windows_x86_64", "os:windows,arch:amd64"),
    ("windows_arm64", "os:windows,arch:aarch64"),
];

/// A JSON value that keeps object keys in file order. Scalars keep their source text, so
/// strings come back out with the escapes Bazel chose.
#[derive(Clone, Debug, PartialEq)]
enum Json {
    Scalar(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            Json::Scalar(_) | Json::Array(_) => None,
        }
    }

    fn entries(&self) -> &[(String, Json)] {
        match self {
            Json::Object(entries) => entries,
            Json::Scalar(_) | Json::Array(_) => &[],
        }
    }

    /// The value under `key`, inserted as an empty object when missing. Bazel keeps its maps
    /// sorted, so a new key goes before the first larger one.
    fn entry(&mut self, key: &str) -> Result<&mut Json, String> {
        let Json::Object(entries) = self else {
            return Err(format!(
                "lock_merge: cannot add {key} to a value that is not an object"
            ));
        };
        let index = match entries.iter().position(|(name, _)| name == key) {
            Some(index) => index,
            None => {
                let index = entries
                    .iter()
                    .position(|(name, _)| name.as_str() > key)
                    .unwrap_or(entries.len());
                entries.insert(index, (key.to_owned(), Json::Object(Vec::new())));
                index
            }
        };
        entries
            .get_mut(index)
            .map(|(_, value)| value)
            .ok_or_else(|| format!("lock_merge: lost the entry {key}"))
    }
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn error(&self, what: &str) -> String {
        format!("{what} at byte {}", self.pos)
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.skip_whitespace();
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", char::from(byte))))
        }
    }

    /// Consumes a string token and returns its source text, quotes included.
    fn string_token(&mut self) -> Result<&'a str, String> {
        let start = self.pos;
        self.pos += 1;
        loop {
            match self.peek() {
                Some(b'\\') => self.pos += 2,
                Some(b'"') => break,
                Some(_) => self.pos += 1,
                None => return Err(self.error("unterminated string")),
            }
        }
        self.pos += 1;
        self.text
            .get(start..self.pos)
            .ok_or_else(|| self.error("string runs past the end"))
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => {
                self.pos += 1;
                let mut entries = Vec::new();
                self.skip_whitespace();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    return Ok(Json::Object(entries));
                }
                loop {
                    self.skip_whitespace();
                    if self.peek() != Some(b'"') {
                        return Err(self.error("expected a key"));
                    }
                    let raw = self.string_token()?;
                    let key: String = serde_json::from_str(raw)
                        .map_err(|error| self.error(&error.to_string()))?;
                    self.expect(b':')?;
                    entries.push((key, self.value()?));
                    self.skip_whitespace();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Json::Object(entries));
                        }
                        _ => return Err(self.error("expected ',' or '}'")),
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_whitespace();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.skip_whitespace();
                    match self.peek() {
                        Some(b',') => self.pos += 1,
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(self.error("expected ',' or ']'")),
                    }
                }
            }
            Some(b'"') => {
                let raw = self.string_token()?;
                serde_json::from_str::<String>(raw)
                    .map_err(|error| self.error(&error.to_string()))?;
                Ok(Json::Scalar(raw.to_owned()))
            }
            Some(_) => {
                let start = self.pos;
                while self
                    .peek()
                    .is_some_and(|byte| !b",]} \t\n\r".contains(&byte))
                {
                    self.pos += 1;
                }
                let raw = self
                    .text
                    .get(start..self.pos)
                    .ok_or_else(|| self.error("bad literal"))?;
                match serde_json::from_str::<serde_json::Value>(raw) {
                    Ok(
                        serde_json::Value::Null
                        | serde_json::Value::Bool(_)
                        | serde_json::Value::Number(_),
                    ) => Ok(Json::Scalar(raw.to_owned())),
                    Ok(
                        serde_json::Value::String(_)
                        | serde_json::Value::Array(_)
                        | serde_json::Value::Object(_),
                    )
                    | Err(_) => Err(self.error(&format!("bad literal {raw}"))),
                }
            }
            None => Err(self.error("unexpected end")),
        }
    }
}

fn parse(text: &str) -> Result<Json, String> {
    let mut parser = Parser { text, pos: 0 };
    let value = parser.value()?;
    parser.skip_whitespace();
    if parser.peek().is_some() {
        return Err(parser.error("trailing text"));
    }
    Ok(value)
}

/// Writes `value` the way Bazel's lock writer does: two-space indent, `"key": value`,
/// empty containers on one line.
fn write(value: &Json, indent: usize, out: &mut String) -> Result<(), String> {
    match value {
        Json::Scalar(raw) => out.push_str(raw),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Object(entries) if entries.is_empty() => out.push_str("{}"),
        Json::Array(items) => {
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(indent + 2));
                write(item, indent + 2, out)?;
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            out.push(']');
        }
        Json::Object(entries) => {
            out.push_str("{\n");
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(indent + 2));
                out.push_str(&serde_json::to_string(key).map_err(|error| error.to_string())?);
                out.push_str(": ");
                write(item, indent + 2, out)?;
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            out.push('}');
        }
    }
    Ok(())
}

fn read_lock(path: &Path) -> Result<Json, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("lock_merge: missing {}: {error}", path.display()))?;
    parse(&text).map_err(|error| format!("lock_merge: {} is not JSON: {error}", path.display()))
}

fn deep_merge(target: &mut Json, source: &Json) -> Result<(), String> {
    match (&*target, source) {
        (Json::Object(_), Json::Object(entries)) => {
            for (key, value) in entries {
                let missing = target.get(key).is_none();
                let slot = target.entry(key)?;
                if missing {
                    *slot = value.clone();
                } else {
                    deep_merge(slot, value)?;
                }
            }
        }
        _ => *target = source.clone(),
    }
    Ok(())
}

fn check_directories(locks: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(locks)
        .map_err(|error| format!("lock_merge: cannot read {}: {error}", locks.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let known = name
            .strip_prefix("lock-")
            .is_some_and(|platform| PLATFORMS.iter().any(|(known, _)| *known == platform));
        if !known {
            return Err(format!("lock_merge: unknown platform directory {name}"));
        }
    }
    Ok(())
}

fn check_keys(platform: &str, lock: &Json) -> Result<(), String> {
    let extensions = lock
        .get("moduleExtensions")
        .map(Json::entries)
        .unwrap_or_default();
    let mut unknown: Vec<&str> = extensions
        .iter()
        .flat_map(|(_, extension)| extension.entries())
        .map(|(key, _)| key.as_str())
        .filter(|key| *key != "general" && PLATFORMS.iter().all(|(_, factor)| factor != key))
        .collect();
    unknown.sort_unstable();
    unknown.dedup();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "lock_merge: lock-{platform} has extension keys that are not general or a platform factor: {}",
            unknown.join(" ")
        ))
    }
}

pub fn merge(args: &[String]) -> Result<(), String> {
    let [locks, out] = args else {
        return Err("usage: producers_bin --lock-merge <locks_dir> <out>".to_owned());
    };
    let locks = Path::new(locks);
    check_directories(locks)?;
    let mut loaded = Vec::new();
    for (platform, factor) in PLATFORMS {
        let lock = read_lock(
            &locks
                .join(format!("lock-{platform}"))
                .join("MODULE.bazel.lock"),
        )?;
        loaded.push((platform, factor, lock));
    }
    for (platform, _, lock) in &loaded {
        check_keys(platform, lock)?;
    }
    let Some(((_, _, first), rest)) = loaded.split_first() else {
        return Err("lock merge has no platforms".to_owned());
    };
    let mut merged = first.clone();
    for (_, factor, lock) in rest {
        let extensions = lock
            .get("moduleExtensions")
            .map(Json::entries)
            .unwrap_or_default();
        for (key, value) in extensions {
            let Some(entry) = value.get(factor) else {
                continue;
            };
            *merged
                .entry("moduleExtensions")?
                .entry(key)?
                .entry(factor)? = entry.clone();
        }
        if let Some(facts) = lock.get("facts") {
            deep_merge(merged.entry("facts")?, facts)?;
        }
    }
    let mut text = String::new();
    write(&merged, 0, &mut text)?;
    text.push('\n');
    std::fs::write(out, text).map_err(|error| format!("lock_merge: cannot write {out}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lock shaped like Bazel's: records in field order, maps sorted, escapes as written.
    const LOCK: &str = r#"{
  "lockFileVersion": 28,
  "registryFileHashes": {
    "https://bcr.bazel.build/a": "sha256-A=",
    "https://bcr.bazel.build/b": "q\"\\ \u00e9"
  },
  "selectedYankedVersions": {},
  "moduleExtensions": {
    "@@rules_go+//go:extensions.bzl%go_sdk": {
      "os:linux,arch:amd64": {
        "bzlTransitiveDigest": "linux",
        "usagesDigest": "u",
        "recordedInputs": [],
        "generatedRepoSpecs": {
          "go_sdk": {
            "repoRuleId": "@@rules_go+//go:sdk.bzl%go_download_sdk_rule",
            "attributes": {
              "patch_strip": 0,
              "experiments": [
                "a",
                "b"
              ],
              "strip": true,
              "none": null
            }
          }
        }
      }
    }
  },
  "facts": {
    "//cc:extensions.bzl%cc": {
      "linux": 1
    }
  },
  "factsVersions": {}
}
"#;

    fn platform_lock(factor: &str, fact: &str) -> String {
        LOCK.replace("os:linux,arch:amd64", factor)
            .replace(
                "\"bzlTransitiveDigest\": \"linux\"",
                &format!("\"bzlTransitiveDigest\": \"{fact}\""),
            )
            .replace("\"linux\": 1", &format!("\"{fact}\": 1"))
    }

    fn merged(locks: &[(&str, String)]) -> Result<String, String> {
        let dir = dx_test_scratch::scratch("lock-merge");
        for (platform, text) in locks {
            let platform_dir = dir.path().join(format!("lock-{platform}"));
            std::fs::create_dir_all(&platform_dir).expect("create platform dir");
            std::fs::write(platform_dir.join("MODULE.bazel.lock"), text).expect("write lock");
        }
        let out = dir.path().join("MODULE.bazel.lock");
        merge(&[
            dir.path().to_string_lossy().into_owned(),
            out.to_string_lossy().into_owned(),
        ])?;
        Ok(std::fs::read_to_string(out).expect("read merged lock"))
    }

    fn all_platforms() -> Vec<(&'static str, String)> {
        PLATFORMS
            .iter()
            .map(|(platform, factor)| (*platform, platform_lock(factor, platform)))
            .collect()
    }

    #[test]
    fn writes_a_bazel_lock_back_unchanged() {
        let mut out = String::new();
        write(&parse(LOCK).expect("parse"), 0, &mut out).expect("write");
        out.push('\n');
        assert_eq!(out, LOCK);
    }

    #[test]
    fn merge_keeps_field_order_and_sorts_new_map_keys() {
        let text = merged(&all_platforms()).expect("merge");
        let keys: Vec<&str> = text
            .lines()
            .filter(|line| line.starts_with("  \""))
            .filter_map(|line| line.split('"').nth(1))
            .collect();
        assert_eq!(
            keys,
            [
                "lockFileVersion",
                "registryFileHashes",
                "selectedYankedVersions",
                "moduleExtensions",
                "facts",
                "factsVersions"
            ]
        );
        let lock = parse(&text).expect("merged lock parses");
        let go_sdk = lock
            .get("moduleExtensions")
            .and_then(|extensions| extensions.get("@@rules_go+//go:extensions.bzl%go_sdk"))
            .expect("go_sdk entry");
        let factors: Vec<&str> = go_sdk
            .entries()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        let mut sorted = factors.clone();
        sorted.sort_unstable();
        assert_eq!(factors, sorted);
        assert_eq!(factors.len(), PLATFORMS.len());
        for (platform, factor) in PLATFORMS {
            let entry = go_sdk.get(factor).expect("factor entry");
            let fields: Vec<&str> = entry
                .entries()
                .iter()
                .map(|(key, _)| key.as_str())
                .collect();
            assert_eq!(
                fields,
                [
                    "bzlTransitiveDigest",
                    "usagesDigest",
                    "recordedInputs",
                    "generatedRepoSpecs"
                ]
            );
            assert_eq!(
                entry.get("bzlTransitiveDigest"),
                Some(&Json::Scalar(format!("\"{platform}\"")))
            );
        }
        let facts = lock
            .get("facts")
            .and_then(|facts| facts.get("//cc:extensions.bzl%cc"))
            .expect("cc facts");
        let mut fact_keys: Vec<&str> = facts
            .entries()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect();
        assert_eq!(fact_keys.len(), PLATFORMS.len());
        let before = fact_keys.clone();
        fact_keys.sort_unstable();
        assert_eq!(before, fact_keys);
        assert!(
            text.contains(r#""q\"\\ \u00e9""#),
            "escapes are kept as written"
        );
        assert!(text.ends_with("}\n"));
    }

    #[test]
    fn merge_of_identical_platform_entries_is_the_lock_itself() {
        let one = all_platforms();
        let first = merged(&one).expect("merge");
        let again: Vec<(&str, String)> = PLATFORMS
            .iter()
            .map(|(platform, _)| (*platform, first.clone()))
            .collect();
        assert_eq!(merged(&again).expect("merge again"), first);
    }

    #[test]
    fn rejects_unknown_extension_keys() {
        let mut locks = all_platforms();
        if let Some((_, text)) = locks.get_mut(1) {
            *text = text.replace("os:linux,arch:aarch64", "os:freebsd,arch:amd64");
        }
        let error = merged(&locks).expect_err("unknown factor");
        assert!(error.contains("os:freebsd,arch:amd64"), "{error}");
    }

    #[test]
    fn rejects_malformed_json() {
        for bad in ["{\"a\": }", "{\"a\": 1", "[1 2]", "{\"a\": nul}", "{} x"] {
            assert!(parse(bad).is_err(), "{bad} should not parse");
        }
    }
}
