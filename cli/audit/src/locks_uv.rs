use crate::vuln::{normalize_pypi_name, LockedPackage};

const PYPI_HOST: &str = "pypi.org";

pub const UV_SET: &str = "uv";

pub fn parse_uv_lock(text: &str) -> Result<Vec<LockedPackage>, String> {
    let root: toml::Table = text
        .parse()
        .map_err(|error: toml::de::Error| format!("invalid uv.lock: {error}"))?;
    if root
        .get("version")
        .and_then(toml::Value::as_integer)
        .is_none()
    {
        return Err("invalid uv.lock: missing version".to_owned());
    }
    let Some(packages) = root.get("package").and_then(toml::Value::as_array) else {
        return Err("invalid uv.lock: missing [[package]]".to_owned());
    };
    let members = workspace_members(&root);
    let mut out = Vec::new();
    for entry in packages {
        let Some(table) = entry.as_table() else {
            return Err("invalid uv.lock: [[package]] is not a table".to_owned());
        };
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "invalid uv.lock: package without a name".to_owned())?
            .to_owned();
        let version = table
            .get("version")
            .and_then(toml::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let source = table
            .get("source")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("invalid uv.lock: package {name} has no source"))?;
        let kind = classify(source, &name, &members)?;
        let (is_git, is_private) = match kind {
            Source::Member => continue,
            Source::Pypi => (false, false),
            Source::Git => (true, false),
            Source::Other => (false, true),
        };
        out.push(LockedPackage {
            is_git,
            is_private,
            ..LockedPackage::new(UV_SET, name, version)
        });
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Source {
    Pypi,
    Git,
    Member,
    Other,
}

fn workspace_members(root: &toml::Table) -> Vec<String> {
    root.get("manifest")
        .and_then(toml::Value::as_table)
        .and_then(|manifest| manifest.get("members"))
        .and_then(toml::Value::as_array)
        .map(|members| {
            members
                .iter()
                .filter_map(toml::Value::as_str)
                .map(normalize_pypi_name)
                .collect()
        })
        .unwrap_or_default()
}

fn classify(source: &toml::Table, name: &str, members: &[String]) -> Result<Source, String> {
    if let Some(registry) = source.get("registry") {
        let Some(url) = registry.as_str() else {
            return Err(format!(
                "invalid uv.lock: package {name} has a bad registry"
            ));
        };
        return Ok(if is_pypi(url) {
            Source::Pypi
        } else {
            Source::Other
        });
    }
    if source.contains_key("git") {
        return Ok(Source::Git);
    }
    for key in ["virtual", "editable"] {
        if let Some(path) = source.get(key).and_then(toml::Value::as_str) {
            let member = path == "." || members.contains(&normalize_pypi_name(name));
            return Ok(if member {
                Source::Member
            } else {
                Source::Other
            });
        }
    }
    for key in ["url", "path", "directory"] {
        if source.contains_key(key) {
            return Ok(Source::Other);
        }
    }
    Err(format!(
        "invalid uv.lock: package {name} has an unknown source"
    ))
}

fn is_pypi(url: &str) -> bool {
    url::Url::parse(url)
        .map(|parsed| {
            parsed.scheme() == "https"
                && parsed.host_str() == Some(PYPI_HOST)
                && parsed.path().trim_end_matches('/') == "/simple"
        })
        .unwrap_or(false)
}
