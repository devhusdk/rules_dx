use crate::discover::{Candidate, Fetch, Findings, LookupError, github, ruby};
use dx_error::DxError;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const SET: &str = "pins";

pub fn listed(workspace: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(workspace) else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(dir) = entry.file_name().into_string() else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(entry.path().join("lang.json")) else {
            continue;
        };
        let Ok(lang) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(pins) = lang.get("pins").and_then(Value::as_array) else {
            continue;
        };
        for pin in pins.iter().filter_map(Value::as_str) {
            let path = std::path::Path::new(pin);
            let inside = path
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)));
            if inside
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                found.push(format!("{dir}/{pin}"));
            }
        }
    }
    found.sort();
    found
}

pub fn outdated(
    path: &str,
    text: &str,
    github_base: &str,
    rubygems_base: &str,
    fetch: &dyn Fetch,
) -> Findings {
    let mut found = Findings::default();
    let problem = |package: &str, reason: String| LookupError {
        set: SET.to_owned(),
        package: package.to_owned(),
        reason,
    };
    let pins: Value = match serde_json::from_str(text) {
        Ok(pins) => pins,
        Err(error) => {
            found.errors.push(problem(path, error.to_string()));
            return found;
        }
    };
    let archives = pins.get("archives").and_then(Value::as_object);
    if pins.get("cells").is_some() || archives.is_none() {
        match version_of(&pins) {
            Ok(current) => match newest_version(&pins, github_base, rubygems_base, fetch) {
                Ok(latest) => push_newer(&mut found, path, current, latest),
                Err(error) => found.errors.push(problem(path, error.to_string())),
            },
            Err(error) => found.errors.push(problem(path, error.to_string())),
        }
    }
    let mut latest: BTreeMap<(String, String), Result<String, String>> = BTreeMap::new();
    for (name, archive) in archives.into_iter().flatten() {
        let Some(followed) = followed_archive(archive) else {
            continue;
        };
        let package = format!("{path}#{name}");
        let key = (followed.repo.clone(), followed.tag_prefix.clone());
        let newest = latest.entry(key).or_insert_with(|| {
            github::latest_tag(&followed.repo, &followed.tag_prefix, github_base, fetch)
                .map_err(|e| e.to_string())
        });
        match newest {
            Ok(newest) => push_newer(&mut found, &package, followed.version, newest.clone()),
            Err(reason) => found.errors.push(problem(&package, reason.clone())),
        }
    }
    found
}

fn push_newer(found: &mut Findings, package: &str, current: &str, latest: String) {
    if latest != current {
        found.candidates.push(Candidate {
            set: SET.to_owned(),
            package: package.to_owned(),
            current: current.to_owned(),
            latest,
            published: None,
            prerelease: false,
            repo: None,
            tags: None,
        });
    }
}

struct FollowedArchive<'a> {
    repo: String,
    tag_prefix: String,
    version: &'a str,
}

fn followed_archive(archive: &Value) -> Option<FollowedArchive<'_>> {
    let version = archive.get("version").and_then(Value::as_str)?;
    let url = first_url(archive).ok()?;
    let path = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = path.split('/').collect();
    let tag = match parts.as_slice() {
        [_, _, "releases", "download", tag, _] => *tag,
        [_, _, "archive", "refs", "tags", file] | [_, _, "archive", file] => {
            [".tar.gz", ".tar.xz", ".tar.bz2", ".tgz", ".zip"]
                .iter()
                .find_map(|ext| file.strip_suffix(ext))?
        }
        _ => return None,
    };
    let (owner, repo) = (parts.first()?, parts.get(1)?);
    let tag_prefix = tag.strip_suffix(version)?;
    if version.is_empty() || owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(FollowedArchive {
        repo: format!("{owner}/{repo}"),
        tag_prefix: tag_prefix.to_owned(),
        version,
    })
}

pub fn split(package: &str) -> (&str, Option<&str>) {
    match package.split_once('#') {
        Some((path, archive)) => (path, Some(archive)),
        None => (package, None),
    }
}

pub fn newest_version(
    pins: &Value,
    github_base: &str,
    rubygems_base: &str,
    fetch: &dyn Fetch,
) -> Result<String, DxError> {
    let (_, cell) = first_cell(pins)?;
    let url = first_url(cell)?;
    if cell["type"] == "gem" {
        let version = version_of(pins)?;
        let name = gem_name(url, version)?;
        let body = fetch.get(&format!("{rubygems_base}/api/v1/versions/{name}.json"))?;
        return ruby::newest(&body, false).map(|(number, _, _)| number);
    }
    let repo = github_repo(url)?;
    let tag = github::latest_tag(&repo, "", github_base, fetch)?;
    Ok(tag.strip_prefix('v').unwrap_or(&tag).to_owned())
}

pub fn rewrite(
    text: &str,
    archive: Option<&str>,
    latest: &str,
    download: &dyn Fn(&str) -> Result<String, DxError>,
) -> Result<Option<String>, DxError> {
    let mut pins: Value =
        serde_json::from_str(text).map_err(|e| DxError::failure(e.to_string()))?;
    let Some(archive) = archive else {
        return rewrite_cells(pins, latest, download);
    };
    let entry = pins
        .get_mut("archives")
        .and_then(|archives| archives.get_mut(archive))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| DxError::failure(format!("pins.json has no archive {archive}")))?;
    let old = entry
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| DxError::failure(format!("archive {archive} has no version")))?
        .to_owned();
    if old == latest {
        return Ok(None);
    }
    let mut digests = BTreeMap::new();
    move_entry(entry, &old, latest, &mut digests, download)?;
    entry.insert("version".to_owned(), Value::String(latest.to_owned()));
    render(&pins).map(Some)
}

fn rewrite_cells(
    mut pins: Value,
    latest: &str,
    download: &dyn Fn(&str) -> Result<String, DxError>,
) -> Result<Option<String>, DxError> {
    let old = version_of(&pins)?.to_owned();
    if old == latest {
        return Ok(None);
    }
    let mut digests: BTreeMap<String, String> = BTreeMap::new();
    let cells = pins
        .get_mut("cells")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| DxError::failure("pins.json has no cells"))?;
    for cell in cells.values_mut().filter_map(Value::as_object_mut) {
        if cell.contains_key("stub") || !cell.contains_key("urls") {
            continue;
        }
        move_entry(cell, &old, latest, &mut digests, download)?;
    }
    pins.as_object_mut()
        .ok_or_else(|| DxError::failure("pins.json is not an object"))?
        .insert("version".to_owned(), Value::String(latest.to_owned()));
    render(&pins).map(Some)
}

fn move_entry(
    entry: &mut Map<String, Value>,
    old: &str,
    latest: &str,
    digests: &mut BTreeMap<String, String>,
    download: &dyn Fn(&str) -> Result<String, DxError>,
) -> Result<(), DxError> {
    let urls: Vec<String> = entry
        .get("urls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|url| url.replace(old, latest))
        .collect();
    let first = urls
        .first()
        .ok_or_else(|| DxError::failure("an entry lists no url"))?;
    let digest = if let Some(digest) = digests.get(first) {
        digest.clone()
    } else {
        let digest = download(first)?;
        digests.insert(first.clone(), digest.clone());
        digest
    };
    entry.insert("sha256".to_owned(), Value::String(digest));
    entry.insert(
        "urls".to_owned(),
        Value::Array(urls.into_iter().map(Value::String).collect()),
    );
    if let Some(prefix) = entry.get("strip_prefix").and_then(Value::as_str) {
        let moved = prefix.replace(old, latest);
        entry.insert("strip_prefix".to_owned(), Value::String(moved));
    }
    Ok(())
}

pub fn render(pins: &Value) -> Result<String, DxError> {
    let mut out =
        serde_json::to_string_pretty(&sorted(pins)).map_err(|e| DxError::failure(e.to_string()))?;
    out.push('\n');
    Ok(out)
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let ordered: BTreeMap<&String, Value> =
                map.iter().map(|(k, v)| (k, sorted(v))).collect();
            let mut out = Map::new();
            for (key, value) in ordered {
                out.insert(key.clone(), value);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other @ (Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)) => {
            other.clone()
        }
    }
}

fn version_of(pins: &Value) -> Result<&str, DxError> {
    pins["version"]
        .as_str()
        .ok_or_else(|| DxError::failure("pins.json has no version"))
}

fn first_cell(pins: &Value) -> Result<(&String, &Value), DxError> {
    pins.get("cells")
        .and_then(Value::as_object)
        .and_then(|cells| cells.iter().find(|(_, cell)| cell.get("stub").is_none()))
        .ok_or_else(|| DxError::failure("pins.json has no downloadable cell"))
}

fn first_url(cell: &Value) -> Result<&str, DxError> {
    cell.get("urls")
        .and_then(|urls| urls.get(0))
        .and_then(Value::as_str)
        .ok_or_else(|| DxError::failure("an entry lists no url"))
}

fn github_repo(url: &str) -> Result<String, DxError> {
    let path = url
        .strip_prefix("https://github.com/")
        .ok_or_else(|| DxError::failure("the first url is not on github.com"))?;
    let mut parts = path.split('/');
    match (parts.next(), parts.next()) {
        (Some(owner), Some(repo)) if !owner.is_empty() && !repo.is_empty() => {
            Ok(format!("{owner}/{repo}"))
        }
        _ => Err(DxError::failure("the first url names no GitHub repository")),
    }
}

fn gem_name<'a>(url: &'a str, version: &str) -> Result<&'a str, DxError> {
    let file = url.rsplit('/').next().unwrap_or(url);
    file.split_once(&format!("-{version}"))
        .map(|(name, _)| name)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| DxError::failure("the gem url does not hold the version"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINS: &str = r#"{
  "cells": {
    "linux_x86_64": {
      "binary": "bin/tool",
      "executable": true,
      "sha256": "aa",
      "urls": [
        "https://github.com/Org/Tool/releases/download/v1.0.0/tool-1.0.0-linux.tar.gz"
      ]
    },
    "macos_arm64": {
      "binary": "bin/tool",
      "sha256": "aa",
      "urls": [
        "https://github.com/Org/Tool/releases/download/v1.0.0/tool-1.0.0-linux.tar.gz"
      ]
    },
    "windows_x86_64": {
      "binary": "tool.exe",
      "stub": "exit 2\n"
    }
  },
  "version": "1.0.0"
}
"#;

    const GEM: &str = r#"{"cells":{"linux_x86_64":{"binary":"b","sha256":"aa","type":"gem","urls":["https://rubygems.org/downloads/sorbet-static-0.6.1-x86_64-linux.gem"]}},"version":"0.6.1"}"#;

    const ARCHIVES: &str = r#"{
  "archives": {
    "lib": {
      "sha256": "aa",
      "strip_prefix": "lib-lib-v2.0.0",
      "urls": [
        "https://github.com/Org/Lib/archive/refs/tags/lib-v2.0.0.tar.gz"
      ],
      "version": "2.0.0"
    },
    "lib_tool": {
      "sha256": "aa",
      "urls": [
        "https://github.com/Org/Lib/releases/download/lib-v2.0.0/tool"
      ],
      "version": "2.0.0"
    },
    "nightly": {
      "sha256": "aa",
      "urls": [
        "https://github.com/Org/Tool/releases/download/10082026/tool-1.0.0.tar.xz"
      ],
      "version": "1.0.0"
    },
    "odd": {
      "sha256": "aa",
      "urls": [
        "https://github.com/Org/Odd/archive/v2.0.0.zip"
      ],
      "version": "2.0.0"
    },
    "unversioned": {
      "sha256": "aa"
    },
    "vendor": {
      "sha256": "aa",
      "urls": [
        "https://example.com/vendor-1.0.0.tar.gz"
      ],
      "version": "1.0.0"
    }
  }
}
"#;

    fn fake(url: &str) -> Result<String, DxError> {
        match url {
            "https://api.github.com/repos/Org/Tool/releases/latest" => {
                Ok(r#"{"tag_name":"v1.2.0","published_at":"2026-01-01T00:00:00Z"}"#.to_owned())
            }
            "https://api.github.com/repos/Org/Lib/releases/latest" => {
                Ok(r#"{"tag_name":"lib-v2.1.0"}"#.to_owned())
            }
            "https://api.github.com/repos/Org/Odd/releases/latest" => {
                Ok(r#"{"tag_name":"3.0.0"}"#.to_owned())
            }
            "https://rubygems.org/api/v1/versions/sorbet-static.json" => Ok(
                r#"[{"number":"0.7.0.pre","prerelease":true},{"number":"0.6.9","prerelease":false},{"number":"0.6.1","prerelease":false}]"#
                    .to_owned(),
            ),
            other => Err(DxError::failure(format!("unexpected {other}"))),
        }
    }

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn github_pins_follow_the_latest_release() {
        let found = newest_version(
            &parse(PINS),
            "https://api.github.com",
            "https://rubygems.org",
            &fake,
        );
        assert_eq!(found.unwrap(), "1.2.0");
    }

    #[test]
    fn gem_pins_follow_the_newest_stable_gem() {
        let found = newest_version(
            &parse(GEM),
            "https://api.github.com",
            "https://rubygems.org",
            &fake,
        );
        assert_eq!(found.unwrap(), "0.6.9");
    }

    #[test]
    fn rewrite_moves_version_urls_and_digests_and_skips_stubs() {
        let seen = std::cell::RefCell::new(Vec::new());
        let download = |url: &str| {
            seen.borrow_mut().push(url.to_owned());
            Ok("bb".repeat(32))
        };
        let out = rewrite(PINS, None, "1.2.0", &download).unwrap().unwrap();
        let want = PINS
            .replace("1.0.0", "1.2.0")
            .replace("\"aa\"", &format!("\"{}\"", "bb".repeat(32)));
        assert_eq!(out, want);
        assert_eq!(seen.borrow().len(), 1);
    }

    #[test]
    fn rewrite_to_the_same_version_changes_nothing() {
        let download = |_: &str| Err(DxError::failure("no download"));
        assert_eq!(rewrite(PINS, None, "1.0.0", &download).unwrap(), None);
    }

    #[test]
    fn outdated_reports_a_newer_release_once() {
        let found = outdated(
            "a/pins.json",
            PINS,
            "https://api.github.com",
            "https://rubygems.org",
            &fake,
        );
        assert_eq!(found.candidates.len(), 1);
        assert_eq!(found.candidates[0].package, "a/pins.json");
        assert_eq!(found.candidates[0].latest, "1.2.0");
        let current = PINS.replace("1.0.0", "1.2.0");
        let found = outdated(
            "a/pins.json",
            &current,
            "https://api.github.com",
            "https://rubygems.org",
            &fake,
        );
        assert!(found.candidates.is_empty() && found.errors.is_empty());
    }

    #[test]
    fn outdated_reports_a_failed_lookup() {
        let found = outdated(
            "a/pins.json",
            PINS,
            "https://api.github.com",
            "https://rubygems.org",
            &|_: &str| Err(DxError::failure("HTTP 500")),
        );
        assert_eq!(found.errors.len(), 1);
    }

    #[test]
    fn listed_reads_json_pins_of_each_lang_json() {
        let root = std::env::temp_dir().join(format!("dx-pins-listed-{}", std::process::id()));
        dx_atomic_fs::discard_dir(&root);
        std::fs::create_dir_all(root.join("b")).unwrap();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(
            root.join("b/lang.json"),
            r#"{"pins":["toolchain/pins.json","x/pins.bzl"]}"#,
        )
        .unwrap();
        std::fs::write(root.join("a/lang.json"), r#"{"pins":["pins.json"]}"#).unwrap();
        assert_eq!(listed(&root), vec!["a/pins.json", "b/toolchain/pins.json"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn render_keeps_sorted_keys_and_a_trailing_newline() {
        assert_eq!(render(&parse(PINS)).unwrap(), PINS);
        let shuffled = r#"{"version":"1.0.0","cells":{"b":{"urls":[],"binary":"x"},"a":{}}}"#;
        let out = render(&parse(shuffled)).unwrap();
        assert!(out.ends_with("}\n"));
        assert!(out.find("\"a\"").unwrap() < out.find("\"b\"").unwrap());
        assert!(out.find("\"binary\"").unwrap() < out.find("\"urls\"").unwrap());
        assert!(out.find("\"cells\"").unwrap() < out.find("\"version\"").unwrap());
    }

    #[test]
    fn a_failed_download_fails_the_rewrite() {
        let download = |_: &str| Err(DxError::failure("HTTP 404"));
        assert!(rewrite(PINS, None, "1.2.0", &download).is_err());
    }

    #[test]
    fn listed_skips_pins_that_leave_their_directory() {
        let root = std::env::temp_dir().join(format!("dx-pins-escape-{}", std::process::id()));
        dx_atomic_fs::discard_dir(&root);
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::write(
            root.join("a/lang.json"),
            r#"{"pins":["../x.json","/abs/x.json","ok/pins.json","./y.json"]}"#,
        )
        .unwrap();
        assert_eq!(listed(&root), vec!["a/ok/pins.json"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn listed_skips_directories_whose_names_are_not_utf8() {
        use std::os::unix::ffi::OsStrExt;
        let root = std::env::temp_dir().join(format!("dx-pins-bytes-{}", std::process::id()));
        dx_atomic_fs::discard_dir(&root);
        let odd = root.join(std::ffi::OsStr::from_bytes(b"bad\xff"));
        std::fs::create_dir_all(&odd).unwrap();
        std::fs::write(odd.join("lang.json"), r#"{"pins":["pins.json"]}"#).unwrap();
        assert_eq!(listed(&root), Vec::<String>::new());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn outdated_follows_each_github_archive_that_names_its_version_in_the_tag() {
        let seen = std::cell::RefCell::new(Vec::new());
        let fetch = |url: &str| {
            seen.borrow_mut().push(url.to_owned());
            fake(url)
        };
        let found = outdated(
            "a/pins.json",
            ARCHIVES,
            "https://api.github.com",
            "https://rubygems.org",
            &fetch,
        );
        let got: Vec<(&str, &str, &str)> = found
            .candidates
            .iter()
            .map(|c| (c.package.as_str(), c.current.as_str(), c.latest.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a/pins.json#lib", "2.0.0", "2.1.0"),
                ("a/pins.json#lib_tool", "2.0.0", "2.1.0"),
            ]
        );
        assert_eq!(found.errors.len(), 1);
        assert_eq!(found.errors[0].package, "a/pins.json#odd");
        assert!(found.errors[0].reason.contains("does not start with \"v\""));
        assert_eq!(seen.borrow().len(), 2);
    }

    #[test]
    fn outdated_checks_cells_and_archives_of_one_file() {
        let both = PINS.replacen(
            "{\n",
            r#"{"archives":{"lib":{"sha256":"aa","urls":["https://github.com/Org/Lib/archive/refs/tags/lib-v2.0.0.tar.gz"],"version":"2.0.0"}},"#,
            1,
        );
        let found = outdated(
            "a/pins.json",
            &both,
            "https://api.github.com",
            "https://rubygems.org",
            &fake,
        );
        let packages: Vec<&str> = found
            .candidates
            .iter()
            .map(|c| c.package.as_str())
            .collect();
        assert_eq!(packages, vec!["a/pins.json", "a/pins.json#lib"]);
        assert!(found.errors.is_empty());
    }

    #[test]
    fn rewrite_moves_one_archive_and_leaves_the_others() {
        let download = |url: &str| {
            assert_eq!(
                url,
                "https://github.com/Org/Lib/archive/refs/tags/lib-v2.1.0.tar.gz"
            );
            Ok("bb".repeat(32))
        };
        let out = rewrite(ARCHIVES, Some("lib"), "2.1.0", &download)
            .unwrap()
            .unwrap();
        let pins = parse(&out);
        let lib = &pins["archives"]["lib"];
        assert_eq!(lib["version"], "2.1.0");
        assert_eq!(lib["strip_prefix"], "lib-lib-v2.1.0");
        assert_eq!(lib["sha256"], "bb".repeat(32));
        assert_eq!(
            pins["archives"]["lib_tool"],
            parse(ARCHIVES)["archives"]["lib_tool"]
        );
        assert_eq!(
            rewrite(ARCHIVES, Some("lib"), "2.0.0", &download).unwrap(),
            None
        );
        assert!(rewrite(ARCHIVES, Some("missing"), "2.1.0", &download).is_err());
        assert!(rewrite(ARCHIVES, Some("unversioned"), "2.1.0", &download).is_err());
    }

    #[test]
    fn rewrite_moves_the_strip_prefix_of_cells() {
        let text = r#"{"cells":{"linux_x86_64":{"sha256":"aa","strip_prefix":"tool-1.0.0-linux","urls":["https://github.com/Org/Tool/releases/download/v1.0.0/tool-1.0.0-linux.tar.xz"]}},"version":"1.0.0"}"#;
        let out = rewrite(text, None, "1.2.0", &|_: &str| Ok("bb".repeat(32)))
            .unwrap()
            .unwrap();
        assert_eq!(
            parse(&out)["cells"]["linux_x86_64"]["strip_prefix"],
            "tool-1.2.0-linux"
        );
    }

    #[test]
    fn split_names_the_file_and_the_archive() {
        assert_eq!(split("a/pins.json"), ("a/pins.json", None));
        assert_eq!(split("a/pins.json#lib"), ("a/pins.json", Some("lib")));
    }
}
