use std::collections::BTreeMap;
use std::path::Path;

use crate::{DepInfo, DepcheckError};

use super::put;

/// Package identity SwiftPM derives from a URL or name: the last path component without
/// `.git`, lowercased.
fn identity(location: &str) -> String {
    let last = location
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    last.strip_suffix(".git").unwrap_or(last).to_lowercase()
}

/// Text between the parentheses of every call that starts with `head`, parentheses in
/// strings and comments aside.
fn calls<'a>(text: &'a str, head: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find(head) {
        let start = from + found + head.len();
        let mut depth = 1;
        let mut in_string = false;
        let mut end = text.len();
        for (offset, ch) in text[start..].char_indices() {
            match ch {
                '"' => in_string = !in_string,
                '(' if !in_string => depth += 1,
                ')' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + offset;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push(&text[start..end]);
        from = start;
    }
    out
}

fn strip_comments(text: &str) -> String {
    text.lines()
        .map(|line| {
            let mut in_string = false;
            let mut previous = ' ';
            for (at, ch) in line.char_indices() {
                if ch == '"' {
                    in_string = !in_string;
                } else if !in_string && ch == '/' && previous == '/' {
                    return &line[..at - 1];
                }
                previous = ch;
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn bump(version: &str, minor: bool) -> String {
    let mut parts = version.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let next_minor = parts.next().unwrap_or(0);
    if minor {
        format!("{major}.{}.0", next_minor + 1)
    } else {
        format!("{}.0.0", major + 1)
    }
}

/// Requirement of one `.package(url:, ...)` call, in the `>= a, < b` grammar.
fn package_spec(args: &str) -> Result<String, DepcheckError> {
    let quoted = r#""([^"]*)""#;
    let pick = |pattern: String| -> Result<Option<String>, DepcheckError> {
        let re = regex::Regex::new(&pattern).map_err(DepcheckError::ManifestRegex)?;
        Ok(re.captures(args).map(|caps| caps[1].to_owned()))
    };
    if let Some(version) = pick(format!(r"\.upToNextMinor\(\s*from:\s*{quoted}"))? {
        return Ok(format!(">= {version}, < {}", bump(&version, true)));
    }
    if let Some(version) = pick(format!(r"\.upToNextMajor\(\s*from:\s*{quoted}"))? {
        return Ok(format!(">= {version}, < {}", bump(&version, false)));
    }
    if let Some(version) = pick(format!(r"\bfrom:\s*{quoted}"))? {
        return Ok(format!(">= {version}, < {}", bump(&version, false)));
    }
    if let Some(version) =
        pick(format!(r"\bexact:\s*{quoted}"))?.or(pick(format!(r"\.exact\(\s*{quoted}"))?)
    {
        return Ok(format!("= {version}"));
    }
    let range = regex::Regex::new(&format!(r"{quoted}\s*(\.\.\.|\.\.<)\s*{quoted}"))
        .map_err(DepcheckError::ManifestRegex)?;
    if let Some(caps) = range.captures(args) {
        let op = if &caps[2] == "..." { "<=" } else { "<" };
        return Ok(format!(">= {}, {op} {}", &caps[1], &caps[3]));
    }
    Ok("*".to_owned())
}

/// Reads a `Package.swift`: remote `.package(url:)` entries and the `.product(name:package:)`
/// entries of the targets. A dependency is keyed by the module name its product is imported
/// as; a package no target takes a product from is keyed by its identity. Path packages are
/// local and skipped.
pub fn parse_swift_manifest(path: &Path) -> Result<BTreeMap<String, DepInfo>, DepcheckError> {
    let text = strip_comments(&std::fs::read_to_string(path).map_err(DepcheckError::ManifestIo)?);
    let url_re =
        regex::Regex::new(r#"\burl:\s*"([^"]+)""#).map_err(DepcheckError::ManifestRegex)?;
    let product_re = regex::Regex::new(r#"name:\s*"([^"]+)"\s*,\s*package:\s*"([^"]+)""#)
        .map_err(DepcheckError::ManifestRegex)?;
    let mut specs: BTreeMap<String, String> = BTreeMap::new();
    for args in calls(&text, ".package(") {
        let Some(caps) = url_re.captures(args) else {
            continue;
        };
        specs.insert(identity(&caps[1]), package_spec(args)?);
    }
    let mut deps = BTreeMap::new();
    let mut referenced = Vec::new();
    for (head, category) in [
        (".target(", "prod"),
        (".executableTarget(", "prod"),
        (".plugin(", "prod"),
        (".testTarget(", "dev"),
    ] {
        for target in calls(&text, head) {
            for caps in product_re.captures_iter(target) {
                let package = identity(&caps[2]);
                let Some(spec) = specs.get(&package) else {
                    continue;
                };
                referenced.push(package.clone());
                put(
                    &mut deps,
                    caps[1].to_owned(),
                    DepInfo {
                        spec: spec.clone(),
                        category: category.to_owned(),
                        optional: false,
                        platform: false,
                        raw: caps[1].to_owned(),
                        peer: false,
                        sha256: String::new(),
                        package,
                    },
                );
            }
        }
    }
    for (package, spec) in &specs {
        if referenced.contains(package) {
            continue;
        }
        deps.insert(
            package.clone(),
            DepInfo {
                spec: spec.clone(),
                category: "prod".to_owned(),
                optional: false,
                platform: false,
                raw: package.clone(),
                peer: false,
                sha256: String::new(),
                package: package.clone(),
            },
        );
    }
    Ok(deps)
}

/// Maps every pinned package identity of a `Package.resolved` (format 1 to 3) to its version,
/// or to its branch or revision when it has none.
pub fn parse_swift_lock(path: &Path) -> Result<BTreeMap<String, String>, DepcheckError> {
    let text = std::fs::read_to_string(path).map_err(DepcheckError::LockIo)?;
    let data: serde_json::Value = serde_json::from_str(&text).map_err(DepcheckError::LockJson)?;
    let pins = data
        .get("pins")
        .or_else(|| data.get("object").and_then(|object| object.get("pins")))
        .and_then(|pins| pins.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = BTreeMap::new();
    for pin in pins {
        let location = ["identity", "package", "location", "repositoryURL"]
            .iter()
            .find_map(|key| pin.get(*key).and_then(|v| v.as_str()));
        let Some(location) = location else {
            continue;
        };
        let state = pin.get("state");
        let version = ["version", "branch", "revision"]
            .iter()
            .find_map(|key| state.and_then(|s| s.get(*key)).and_then(|v| v.as_str()));
        out.insert(identity(location), version.unwrap_or("").to_owned());
    }
    Ok(out)
}

/// Module names an unreferenced pinned package may be imported as: the library products of its
/// checkout under `.build/checkouts` when present, else its identity in camel case.
pub fn module_candidates(lock: &Path, package: &str) -> Vec<String> {
    let manifest = lock.parent().map(|dir| {
        dir.join(".build/checkouts")
            .join(package)
            .join("Package.swift")
    });
    let text = manifest
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    let from_checkout: Vec<String> = match regex::Regex::new(r#"\.library\(\s*name:\s*"([^"]+)""#) {
        Ok(re) => re
            .captures_iter(&text)
            .map(|caps| caps[1].to_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    if !from_checkout.is_empty() {
        return from_checkout;
    }
    let camel = |name: &str| -> String {
        name.split(['-', '_', '.'])
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default()
            })
            .collect()
    };
    let mut names = vec![camel(package)];
    if let Some(rest) = package.strip_prefix("swift-") {
        names.push(camel(rest));
    }
    names
}
