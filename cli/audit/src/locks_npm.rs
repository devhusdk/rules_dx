use crate::vuln::LockedPackage;

pub fn is_npm_git_reference(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("git+") || lower.contains("git://") || lower.contains("git@") {
        return true;
    }
    for prefix in [
        "github:",
        "gitlab:",
        "bitbucket:",
        "gist:",
        "git://",
        "git+",
        "git@",
    ] {
        if lower.starts_with(prefix) {
            return true;
        }
    }
    if lower.contains("codeload.github.com") || lower.contains("/tarball/") {
        return true;
    }
    let without_fragment = lower.split(['#', '?']).next().unwrap_or(&lower);
    if without_fragment.ends_with(".git") || without_fragment.contains(".git/") {
        return true;
    }
    false
}

pub(crate) fn package_lock_name(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let last = trimmed.rsplit("node_modules/").next()?.trim();
    if last.is_empty() {
        return None;
    }
    if last.starts_with('@') && !last.contains('/') {
        return None;
    }
    if last.contains('/') && !last.starts_with('@') {
        let segment = last.rsplit('/').next()?.trim();
        if segment.is_empty() {
            return None;
        }
        return Some(segment.to_owned());
    }
    if last.contains('/') {
        let parts: Vec<&str> = last.split('/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(last.to_owned());
        }
        let segment = parts.last()?.trim();
        if segment.is_empty() {
            return None;
        }
        return Some(segment.to_owned());
    }
    Some(last.to_owned())
}

pub fn parse_package_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("invalid package-lock.json: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "invalid package-lock.json: expected object".to_owned())?;
    let mut out = Vec::new();
    if let Some(packages) = object.get("packages").and_then(|value| value.as_object()) {
        for (path, detail) in packages {
            if path.trim().is_empty() {
                continue;
            }
            let Some(name) = package_lock_name(path) else {
                continue;
            };
            let detail = match detail.as_object() {
                Some(detail) => detail,
                None => continue,
            };
            if detail
                .get("link")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                continue;
            }
            let version = detail
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            let resolved = detail
                .get("resolved")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            let from = detail
                .get("from")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            if version.is_empty() {
                continue;
            }
            if version.starts_with("file:") || version.starts_with("link:") {
                continue;
            }
            if resolved.starts_with("file:") {
                continue;
            }
            let is_git = is_npm_git_reference(&version)
                || is_npm_git_reference(&resolved)
                || is_npm_git_reference(&from);
            out.push(LockedPackage {
                name,
                version,
                set: "npm".to_owned(),
                is_git,
                is_private: false,
            });
        }
    }
    if let Some(dependencies) = object
        .get("dependencies")
        .and_then(|value| value.as_object())
    {
        for (name, detail) in dependencies {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let detail = match detail.as_object() {
                Some(detail) => detail,
                None => continue,
            };
            if detail
                .get("link")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                continue;
            }
            let version = detail
                .get("version")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            let resolved = detail
                .get("resolved")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            let from = detail
                .get("from")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .trim()
                .to_owned();
            if version.is_empty() {
                continue;
            }
            if version.starts_with("file:") || version.starts_with("link:") {
                continue;
            }
            if resolved.starts_with("file:") {
                continue;
            }
            let is_git = is_npm_git_reference(&version)
                || is_npm_git_reference(&resolved)
                || is_npm_git_reference(&from);
            if out.iter().any(|package| {
                package.name == name && package.version == version && package.is_git == is_git
            }) {
                continue;
            }
            out.push(LockedPackage {
                name: name.to_owned(),
                version,
                set: "npm".to_owned(),
                is_git,
                is_private: false,
            });
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version, a.is_git).cmp(&(&b.name, &b.version, b.is_git)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version && a.is_git == b.is_git);
    Ok(out)
}
