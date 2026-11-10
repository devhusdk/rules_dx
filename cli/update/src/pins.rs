//! Moves a `pins.json` to the newest release: version, urls and sha256 of every cell.

use crate::discover::{Fetch, github, ruby};
use dx_error::DxError;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Finds the newest version of the pinned tool, from GitHub releases or rubygems.
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
    let tag = github::latest_tag(&repo, github_base, fetch)?;
    Ok(tag.strip_prefix('v').unwrap_or(&tag).to_owned())
}

/// Rewrites `text` to `latest`, downloading each cell for its sha256.
///
/// `download` returns the sha256 of a url's bytes. Returns `None` when the pins already
/// hold `latest`. Cells with a `stub` are left alone.
pub fn rewrite(
    text: &str,
    latest: &str,
    download: &dyn Fn(&str) -> Result<String, DxError>,
) -> Result<Option<String>, DxError> {
    let mut pins: Value = serde_json::from_str(text).map_err(|e| DxError::failure(e.to_string()))?;
    let old = version_of(&pins)?.to_owned();
    if old == latest {
        return Ok(None);
    }
    let mut digests: BTreeMap<String, String> = BTreeMap::new();
    let cells = pins["cells"]
        .as_object_mut()
        .ok_or_else(|| DxError::failure("pins.json has no cells"))?;
    for cell in cells.values_mut().filter_map(Value::as_object_mut) {
        if cell.contains_key("stub") {
            continue;
        }
        let Some(urls) = cell.get("urls").and_then(Value::as_array) else {
            continue;
        };
        let urls: Vec<String> = urls
            .iter()
            .filter_map(Value::as_str)
            .map(|url| url.replace(&old, latest))
            .collect();
        let first = urls
            .first()
            .ok_or_else(|| DxError::failure("a cell lists no url"))?;
        if !digests.contains_key(first) {
            digests.insert(first.clone(), download(first)?);
        }
        cell.insert("sha256".to_owned(), Value::String(digests[first].clone()));
        cell.insert(
            "urls".to_owned(),
            Value::Array(urls.into_iter().map(Value::String).collect()),
        );
    }
    pins["version"] = Value::String(latest.to_owned());
    render(&pins).map(Some)
}

/// Serializes with sorted keys, 2-space indent and a trailing newline.
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
        other => other.clone(),
    }
}

fn version_of(pins: &Value) -> Result<&str, DxError> {
    pins["version"]
        .as_str()
        .ok_or_else(|| DxError::failure("pins.json has no version"))
}

fn first_cell(pins: &Value) -> Result<(&String, &Value), DxError> {
    pins["cells"]
        .as_object()
        .and_then(|cells| cells.iter().find(|(_, cell)| cell.get("stub").is_none()))
        .ok_or_else(|| DxError::failure("pins.json has no downloadable cell"))
}

fn first_url(cell: &Value) -> Result<&str, DxError> {
    cell["urls"][0]
        .as_str()
        .ok_or_else(|| DxError::failure("a cell lists no url"))
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

    fn fake(url: &str) -> Result<String, DxError> {
        match url {
            "https://api.github.com/repos/Org/Tool/releases/latest" => {
                Ok(r#"{"tag_name":"v1.2.0","published_at":"2026-01-01T00:00:00Z"}"#.to_owned())
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
        let out = rewrite(PINS, "1.2.0", &download).unwrap().unwrap();
        let want = PINS
            .replace("1.0.0", "1.2.0")
            .replace("\"aa\"", &format!("\"{}\"", "bb".repeat(32)));
        assert_eq!(out, want);
        assert_eq!(seen.borrow().len(), 1);
    }

    #[test]
    fn rewrite_to_the_same_version_changes_nothing() {
        let download = |_: &str| Err(DxError::failure("no download"));
        assert_eq!(rewrite(PINS, "1.0.0", &download).unwrap(), None);
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
        assert!(rewrite(PINS, "1.2.0", &download).is_err());
    }
}
