use regex::Regex;
use std::sync::LazyLock;

use super::is_npm_git_reference;

use yarn_lock_parser::parse_str;

use crate::vuln::LockedPackage;

fn pnpm_resolution_is_git(resolution: &yaml_serde::Value) -> bool {
    let mapping = match resolution.as_mapping() {
        Some(mapping) => mapping,
        None => return false,
    };
    let get_str = |key: &str| -> Option<String> {
        mapping.iter().find_map(|(key_value, value)| {
            if key_value.as_str()? == key {
                value.as_str().map(str::to_owned)
            } else {
                None
            }
        })
    };
    if let Some(kind) = get_str("type") {
        if kind.trim().eq_ignore_ascii_case("git") {
            return true;
        }
    }
    if let Some(commit) = get_str("commit") {
        if !commit.trim().is_empty() {
            return true;
        }
    }
    if let Some(repo) = get_str("repo") {
        if is_npm_git_reference(&repo) {
            return true;
        }
    }
    if let Some(tarball) = get_str("tarball") {
        if is_npm_git_reference(&tarball) {
            return true;
        }
    }
    false
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PnpmKeyStyle {
    /// lockfileVersion 5.x: `/name/version_peers`.
    V5,
    /// lockfileVersion 6 and later, or no version: `name@version(peers)`, with a `/` prefix before 9.
    Modern,
}

fn pnpm_key_style(value: &yaml_serde::Value) -> Result<PnpmKeyStyle, String> {
    let version = match value.get("lockfileVersion") {
        None | Some(yaml_serde::Value::Null) => return Ok(PnpmKeyStyle::Modern),
        Some(version) => version,
    };
    let invalid = || format!("invalid pnpm-lock.yaml: unsupported lockfileVersion {version:?}");
    let major = if let Some(text) = version.as_str() {
        text.trim()
            .split('.')
            .next()
            .and_then(|major| major.parse::<u64>().ok())
            .ok_or_else(invalid)?
    } else if let Some(number) = version.as_f64() {
        if !number.is_finite() || number < 0.0 {
            return Err(invalid());
        }
        number.floor() as u64
    } else {
        return Err(invalid());
    };
    Ok(if major < 6 {
        PnpmKeyStyle::V5
    } else {
        PnpmKeyStyle::Modern
    })
}

fn resolve_pnpm_key(
    key: &str,
    detail: &yaml_serde::Value,
    style: PnpmKeyStyle,
) -> Result<(String, String), String> {
    let unresolved = || format!("invalid pnpm-lock.yaml: cannot resolve package key `{key}`");
    let detail_str = |field: &str| {
        detail
            .get(field)
            .and_then(yaml_serde::Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let detail_identity = || Some((detail_str("name")?, detail_str("version")?));
    let resolved = match style {
        PnpmKeyStyle::V5 => match key.strip_prefix('/') {
            Some(rest) => detail_identity().or_else(|| split_pnpm_v5_key(rest)),
            None => detail_identity(),
        },
        PnpmKeyStyle::Modern => match key.strip_prefix('/') {
            Some(rest) => detail_identity().or_else(|| split_pnpm_key(rest)),
            None => split_pnpm_key(key).or_else(detail_identity),
        },
    };
    match resolved {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => Ok((name, version)),
        _ => Err(unresolved()),
    }
}

fn pnpm_packages_from_value(
    value: &yaml_serde::Value,
    style: PnpmKeyStyle,
    out: &mut Vec<LockedPackage>,
) -> Result<(), String> {
    let packages = match value.get("packages") {
        None | Some(yaml_serde::Value::Null) => return Ok(()),
        Some(packages) => packages,
    };
    let mapping = match packages.as_mapping() {
        Some(mapping) => mapping,
        None => return Ok(()),
    };
    for (key_value, detail) in mapping {
        let key = match key_value.as_str().map(str::trim) {
            Some(key) if !key.is_empty() => key.to_owned(),
            _ => {
                return Err(format!(
                    "invalid pnpm-lock.yaml: unusable package key {key_value:?}"
                ))
            }
        };
        if key.contains("link:") || key.contains("file:") {
            continue;
        }
        let (name, version) = resolve_pnpm_key(&key, detail, style)?;
        if version.starts_with("link:") || version.starts_with("file:") {
            continue;
        }
        let is_git = is_npm_git_reference(&key)
            || is_npm_git_reference(&version)
            || detail.get("resolution").is_some_and(pnpm_resolution_is_git);
        out.push(LockedPackage {
            is_git,
            ..LockedPackage::new("npm", name, version)
        });
    }
    Ok(())
}

fn split_pnpm_documents(text: &str) -> Vec<String> {
    if !text
        .lines()
        .any(|line| line.trim_start().starts_with("---"))
    {
        return vec![text.to_owned()];
    }
    let mut documents = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim() == "---" || line.trim_start().starts_with("--- ") {
            if !current.trim().is_empty() {
                documents.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        documents.push(current);
    }
    if documents.is_empty() {
        documents.push(text.to_owned());
    }
    documents
}

pub fn parse_pnpm_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut parsed_any = false;
    for document in split_pnpm_documents(text) {
        if document.trim().is_empty() {
            continue;
        }
        let value: yaml_serde::Value = yaml_serde::from_str(&document)
            .map_err(|error| format!("invalid pnpm-lock.yaml: {error}"))?;
        if value.is_null() {
            continue;
        }
        parsed_any = true;
        if value.get("packages").is_some() {
            let style = pnpm_key_style(&value)?;
            pnpm_packages_from_value(&value, style, &mut out)?;
        } else if value.as_mapping().is_none() {
            return Err("invalid pnpm-lock.yaml: missing packages".to_owned());
        }
    }
    if !parsed_any {
        return Ok(Vec::new());
    }
    if out.is_empty() {
        let single: Result<yaml_serde::Value, _> = yaml_serde::from_str(text);
        if let Ok(value) = single {
            if let Some(packages) = value.get("packages") {
                if !packages.is_null() && packages.as_mapping().is_none() {
                    return Err("invalid pnpm-lock.yaml: missing packages".to_owned());
                }
            }
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version, a.is_git).cmp(&(&b.name, &b.version, b.is_git)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version && a.is_git == b.is_git);
    Ok(out)
}

const YARN_V1_HEADER: &str = "# synthetic\n# yarn lockfile v1\n\n";

fn yarn_stanzas(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            out.push(line.to_owned());
        } else if let Some(last) = out.last_mut() {
            last.push('\n');
            last.push_str(line);
        }
    }
    out
}

fn yarn_field_value(line: &str) -> Option<(&str, String)> {
    let body = line.trim_start();
    for key in ["resolution", "resolved", "version"] {
        let Some(rest) = body.strip_prefix(key) else {
            continue;
        };
        let rest = match rest.chars().next() {
            Some(':') => rest[1..].trim_start(),
            Some(char) if char.is_whitespace() => rest.trim_start(),
            _ => continue,
        };
        if rest.is_empty() {
            continue;
        }
        let value = match rest.strip_prefix('"') {
            Some(quoted) => quoted.split('"').next().unwrap_or_default(),
            None => rest.split_whitespace().next().unwrap_or_default(),
        };
        return Some((key, value.to_owned()));
    }
    None
}

fn yarn_entry_text(stanza: &str) -> String {
    let mut out = String::from(YARN_V1_HEADER);
    for (index, raw) in stanza.lines().enumerate() {
        let line = raw.trim_end();
        if index == 0 {
            let head = line.trim().trim_end_matches(':');
            if head.contains('@') {
                out.push_str(line);
            } else {
                out.push_str(head);
                out.push_str("@npm:0.0.0:");
            }
        } else if let Some((key, value)) = yarn_field_value(line) {
            out.push_str(&line[..line.len() - line.trim_start().len()]);
            out.push_str(key);
            out.push_str(" \"");
            out.push_str(&value);
            out.push('"');
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

fn yarn_is_local_protocol(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("file:") || value.starts_with("link:") || value.starts_with("portal:")
}

fn yarn_entry_package(entry: &yarn_lock_parser::Entry) -> Option<LockedPackage> {
    let name = entry.name.trim();
    let version = entry.version.trim();
    let resolved = entry.resolved.trim();
    if name.is_empty() || version.is_empty() {
        return None;
    }
    if yarn_is_local_protocol(version) || yarn_is_local_protocol(resolved) {
        return None;
    }
    let is_git = entry
        .descriptors
        .iter()
        .any(|(_, range)| is_npm_git_reference(range))
        || is_npm_git_reference(version)
        || is_npm_git_reference(resolved);
    Some(LockedPackage {
        is_git,
        ..LockedPackage::new("npm", name, version)
    })
}

pub fn parse_yarn_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut out = Vec::new();
    for stanza in yarn_stanzas(text) {
        if stanza.trim_end().trim_end_matches(':').trim() == "__metadata" {
            continue;
        }
        let entry_text = yarn_entry_text(&stanza);
        let Ok(lock) = parse_str(&entry_text) else {
            continue;
        };
        for entry in &lock.entries {
            if let Some(package) = yarn_entry_package(entry) {
                out.push(package);
            }
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version, a.is_git).cmp(&(&b.name, &b.version, b.is_git)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version && a.is_git == b.is_git);
    Ok(out)
}

/// Splits a lockfileVersion 5 key after its leading `/`: `name/version_peers`, `@scope/name/version_peers`.
pub(crate) fn split_pnpm_v5_key(rest: &str) -> Option<(String, String)> {
    let rest = rest.trim();
    let mut parts = rest.splitn(if rest.starts_with('@') { 3 } else { 2 }, '/');
    let name = if rest.starts_with('@') {
        let scope = parts.next()?;
        let pkg = parts.next()?;
        format!("{scope}/{pkg}")
    } else {
        parts.next()?.to_owned()
    };
    let tail = parts.next()?;
    let version = tail.split('_').next().unwrap_or("").trim();
    if name.is_empty()
        || name == "@"
        || name.ends_with('/')
        || name.starts_with("@/")
        || version.is_empty()
        || version.contains('/')
    {
        return None;
    }
    Some((name, version.to_owned()))
}

fn scoped_pnpm_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"^@(?P<scope>[^/]+)/(?P<name>.+)@(?P<version>[^@]+)$").ok());
    RE.as_ref()
}

fn unscoped_pnpm_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"^(?P<name>.+)@(?P<version>[^@]+)$").ok());
    RE.as_ref()
}

pub(crate) fn split_pnpm_key(key: &str) -> Option<(String, String)> {
    let base = key
        .split_once('(')
        .map(|(stem, _)| stem)
        .unwrap_or(key)
        .trim();
    if base.is_empty() {
        return None;
    }
    if base.starts_with('@') {
        if let Some(re) = scoped_pnpm_re() {
            if let Some(caps) = re.captures(base) {
                let name = format!("@{}/{}", &caps["scope"], &caps["name"])
                    .trim()
                    .to_owned();
                let version = caps["version"].trim().to_owned();
                if !name.is_empty() && !version.is_empty() {
                    return Some((name, version));
                }
                return None;
            }
            return split_pnpm_scoped_fallback(base);
        }
        return split_pnpm_scoped_fallback(base);
    }
    if let Some(re) = unscoped_pnpm_re() {
        if let Some(caps) = re.captures(base) {
            let name = caps["name"].trim().to_owned();
            let version = caps["version"].trim().to_owned();
            if !name.is_empty() && !version.is_empty() {
                return Some((name, version));
            }
            return None;
        }
        return None;
    }
    split_pnpm_unscoped_fallback(base)
}

pub(crate) fn split_pnpm_scoped_fallback(base: &str) -> Option<(String, String)> {
    let slash = base.find('/')?;
    let rest = &base[slash + 1..];
    let at = rest.rfind('@')?;
    let scope = &base[..slash];
    let name_base = &rest[..at];
    let version = &rest[at + 1..];
    let name = format!("{scope}/{name_base}").trim().to_owned();
    let version = version.trim().to_owned();
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name, version))
}

pub(crate) fn split_pnpm_unscoped_fallback(base: &str) -> Option<(String, String)> {
    let at = base.rfind('@')?;
    let name = base[..at].trim().to_owned();
    let version = base[at + 1..].trim().to_owned();
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some((name, version))
}
