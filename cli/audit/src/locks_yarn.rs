use super::{is_npm_git_reference, split_pnpm_key};

use crate::vuln::LockedPackage;

fn split_yarn_selector(selector: &str) -> Option<String> {
    let trimmed = selector.trim().trim_matches('"').trim();
    if trimmed.is_empty() || trimmed.starts_with("__metadata:") {
        return None;
    }
    if let Some((name, _)) = split_pnpm_key(trimmed) {
        if !name.is_empty() {
            return Some(name);
        }
        return None;
    }
    if !trimmed.contains('@') {
        let name = trimmed.trim_end_matches(':').trim().to_owned();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

fn yarn_field(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with(key) {
        return None;
    }
    let rest = trimmed[key.len()..].trim();
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim();
    if rest.is_empty() {
        return None;
    }
    if let Some(stripped) = rest.strip_prefix('"') {
        let inner = stripped.split('"').next().unwrap_or("").trim();
        if inner.is_empty() {
            return None;
        }
        return Some(inner.to_owned());
    }
    let token = rest.split_whitespace().next().unwrap_or("").trim();
    if token.is_empty() {
        return None;
    }
    Some(token.to_owned())
}

pub fn parse_yarn_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let mut out = Vec::new();
    let mut header: Option<String> = None;
    let mut version: Option<String> = None;
    let mut resolved = String::new();
    let flush = |header: &mut Option<String>,
                 version: &mut Option<String>,
                 resolved: &mut String,
                 out: &mut Vec<LockedPackage>| {
        let Some(selector) = header.take() else {
            *version = None;
            resolved.clear();
            return;
        };
        let version_value = version.take().unwrap_or_default();
        let resolved_value = std::mem::take(resolved);
        let first = selector.split(',').next().unwrap_or("").trim();
        let Some(name) = split_yarn_selector(first) else {
            return;
        };
        if name.is_empty() || version_value.trim().is_empty() {
            return;
        }
        let version_value = version_value.trim().to_owned();
        let resolved_value = resolved_value.trim().to_owned();
        if version_value.starts_with("file:")
            || version_value.starts_with("link:")
            || version_value.starts_with("portal:")
        {
            return;
        }
        if resolved_value.starts_with("file:")
            || resolved_value.starts_with("link:")
            || resolved_value.starts_with("portal:")
        {
            return;
        }
        let is_git = is_npm_git_reference(first)
            || is_npm_git_reference(&version_value)
            || is_npm_git_reference(&resolved_value);
        out.push(LockedPackage {
            name,
            version: version_value,
            set: "npm".to_owned(),
            is_git,
            is_private: false,
        });
    };
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            if header.is_some() && version.is_some() {
                flush(&mut header, &mut version, &mut resolved, &mut out);
            } else if line.trim().is_empty() {
                header = None;
                version = None;
                resolved.clear();
            }
            continue;
        }
        if !line.starts_with([' ', '\t']) {
            if header.is_some() && version.is_some() {
                flush(&mut header, &mut version, &mut resolved, &mut out);
            } else if header.is_some() {
                resolved.clear();
            }
            let selector = line.trim_end_matches(':').trim().to_owned();
            if selector.is_empty() {
                header = None;
                continue;
            }
            header = Some(selector);
            version = None;
            resolved.clear();
            continue;
        }
        if header.is_none() {
            continue;
        }
        if let Some(value) = yarn_field(line, "version") {
            version = Some(value);
        } else if let Some(value) = yarn_field(line, "resolved") {
            resolved = value;
        }
    }
    if header.is_some() && version.is_some() {
        flush(&mut header, &mut version, &mut resolved, &mut out);
    }
    out.sort_by(|a, b| (&a.name, &a.version, a.is_git).cmp(&(&b.name, &b.version, b.is_git)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version && a.is_git == b.is_git);
    Ok(out)
}
