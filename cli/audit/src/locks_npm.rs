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

fn valid_package_name(name: &str) -> bool {
    let unscoped = |part: &str| {
        !part.is_empty()
            && !part.starts_with(['.', '_'])
            && !part.contains(['/', '\\', ' ', '@', ':', '#', '?'])
    };
    match name.strip_prefix('@') {
        Some(rest) => match rest.split_once('/') {
            Some((scope, package)) => unscoped(scope) && unscoped(package),
            None => false,
        },
        None => unscoped(name),
    }
}

pub(crate) fn package_lock_identity(
    path: &str,
    detail: &serde_json::Map<String, serde_json::Value>,
) -> Result<Option<String>, String> {
    match detail.get("name") {
        None => Ok(package_lock_name(path)),
        Some(value) => {
            let name = value
                .as_str()
                .ok_or_else(|| {
                    format!("invalid package-lock.json: name of {path} is not a string")
                })?
                .trim();
            if !valid_package_name(name) {
                return Err(format!(
                    "invalid package-lock.json: invalid package name {name:?} for {path}"
                ));
            }
            Ok(Some(name.to_owned()))
        }
    }
}

pub(crate) fn legacy_alias_identity(name: &str, version: &str) -> Result<(String, String), String> {
    let Some(spec) = version.strip_prefix("npm:") else {
        return Ok((name.to_owned(), version.to_owned()));
    };
    let split = spec
        .char_indices()
        .skip(1)
        .find(|(_, c)| *c == '@')
        .map(|(index, _)| index);
    let (real, real_version) = match split {
        Some(index) => (&spec[..index], spec[index + 1..].trim()),
        None => (spec, ""),
    };
    if !valid_package_name(real) || real_version.is_empty() {
        return Err(format!(
            "invalid package-lock.json: invalid npm alias {version:?} for {name}"
        ));
    }
    Ok((real.to_owned(), real_version.to_owned()))
}

fn entry_text(
    detail: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    path: &str,
) -> Result<String, String> {
    match detail.get(key) {
        None => Ok(String::new()),
        Some(value) => value
            .as_str()
            .map(|text| text.trim().to_owned())
            .ok_or_else(|| format!("invalid package-lock.json: {key} of {path} is not a string")),
    }
}

const MAX_LEGACY_DEPTH: usize = 64;

fn collect_legacy(
    dependencies: &serde_json::Value,
    depth: usize,
    out: &mut Vec<LockedPackage>,
) -> Result<(), String> {
    if depth >= MAX_LEGACY_DEPTH {
        return Err(format!(
            "invalid package-lock.json: dependencies nested deeper than {MAX_LEGACY_DEPTH}"
        ));
    }
    let dependencies = dependencies
        .as_object()
        .ok_or_else(|| "invalid package-lock.json: dependencies is not an object".to_owned())?;
    for (name, detail) in dependencies {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let detail = detail
            .as_object()
            .ok_or_else(|| format!("invalid package-lock.json: entry {name} is not an object"))?;
        if let Some(children) = detail.get("dependencies") {
            collect_legacy(children, depth + 1, out)?;
        }
        if detail
            .get("link")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        let version = entry_text(detail, "version", name)?;
        let resolved = entry_text(detail, "resolved", name)?;
        let from = entry_text(detail, "from", name)?;
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
        let (identity, version) = if is_git {
            (name.to_owned(), version)
        } else {
            legacy_alias_identity(name, &version)?
        };
        let location = (identity != name).then(|| name.to_owned());
        if out.iter().any(|package| {
            package.name == identity && package.version == version && package.is_git == is_git
        }) {
            continue;
        }
        out.push(LockedPackage {
            is_git,
            location,
            ..LockedPackage::new("npm", identity, version)
        });
    }
    Ok(())
}

pub fn parse_package_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| format!("invalid package-lock.json: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "invalid package-lock.json: expected object".to_owned())?;
    let mut out = Vec::new();
    if let Some(packages) = object.get("packages") {
        let packages = packages
            .as_object()
            .ok_or_else(|| "invalid package-lock.json: packages is not an object".to_owned())?;
        for (path, detail) in packages {
            if path.trim().is_empty() {
                continue;
            }
            let detail = detail.as_object().ok_or_else(|| {
                format!("invalid package-lock.json: entry {path} is not an object")
            })?;
            let Some(name) = package_lock_identity(path, detail)? else {
                continue;
            };
            if detail
                .get("link")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
            {
                continue;
            }
            let version = entry_text(detail, "version", path)?;
            let resolved = entry_text(detail, "resolved", path)?;
            let from = entry_text(detail, "from", path)?;
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
                is_git,
                location: Some(path.trim().to_owned()),
                ..LockedPackage::new("npm", name, version)
            });
        }
    }
    if let Some(dependencies) = object.get("dependencies") {
        collect_legacy(dependencies, 0, &mut out)?;
    }
    out.sort_by(|a, b| (&a.name, &a.version, a.is_git).cmp(&(&b.name, &b.version, b.is_git)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version && a.is_git == b.is_git);
    Ok(out)
}
