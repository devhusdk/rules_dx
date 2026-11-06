use super::{Advisory, AdvisoryKind, is_version_family};

/// The set opam advisories belong to. The osv crate has no opam ecosystem, so those entries are
/// read under `GHC`, an ecosystem no snapshot family downloads.
const OPAM_SET: &str = "opam";

pub fn parse_snapshot(text: &str) -> Result<Vec<Advisory>, String> {
    if let Ok(vulns) = serde_json::from_str::<Vec<osv::schema::Vulnerability>>(text) {
        return Ok(project_osv_snapshot(&vulns));
    }
    if text.contains("\"opam\"")
        && let Ok(vulns) = parse_opam_vulnerabilities(text)
    {
        return Ok(project_osv_snapshot(&vulns));
    }
    serde_json::from_str(text).map_err(|error| format!("invalid advisory snapshot: {error}"))
}

fn parse_opam_vulnerabilities(text: &str) -> serde_json::Result<Vec<osv::schema::Vulnerability>> {
    let mut entries: Vec<serde_json::Value> = serde_json::from_str(text)?;
    for entry in &mut entries {
        let affected = entry
            .get_mut("affected")
            .and_then(|value| value.as_array_mut());
        for item in affected.into_iter().flatten() {
            if let Some(ecosystem) = item.pointer_mut("/package/ecosystem")
                && ecosystem == "opam"
            {
                *ecosystem = serde_json::Value::from("GHC");
            }
        }
    }
    serde_json::from_value(serde_json::Value::Array(entries))
}

fn ecosystem_to_set(ecosystem: &osv::schema::Ecosystem) -> Option<&'static str> {
    match ecosystem {
        osv::schema::Ecosystem::CratesIO => Some("cargo"),
        osv::schema::Ecosystem::Npm => Some("npm"),
        osv::schema::Ecosystem::Go => Some("go"),
        osv::schema::Ecosystem::Maven(_) => Some("maven"),
        osv::schema::Ecosystem::NuGet => Some("nuget"),
        osv::schema::Ecosystem::RubyGems => Some("ruby"),
        osv::schema::Ecosystem::PyPI => Some("uv"),
        osv::schema::Ecosystem::Hex => Some("hex"),
        osv::schema::Ecosystem::Pub => Some("pub"),
        osv::schema::Ecosystem::Packagist => Some("packagist"),
        osv::schema::Ecosystem::Hackage => Some("hackage"),
        osv::schema::Ecosystem::SwiftURL => Some("swifturl"),
        osv::schema::Ecosystem::Julia => Some("julia"),
        osv::schema::Ecosystem::CRAN => Some("cran"),
        osv::schema::Ecosystem::GHC => Some(OPAM_SET),
        _ => None,
    }
}

fn known_severity_word(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.to_ascii_lowercase().as_str() {
        "critical" | "high" | "medium" | "low" | "moderate" => Some(trimmed.to_owned()),
        _ => None,
    }
}

fn osv_severity_text(
    vuln: &osv::schema::Vulnerability,
    affected: &osv::schema::Affected,
) -> String {
    if let Some(list) = affected.severity.as_ref() {
        for entry in list {
            if let Some(word) = known_severity_word(&entry.score) {
                return word;
            }
        }
    }
    if let Some(list) = vuln.severity.as_ref() {
        for entry in list {
            if let Some(word) = known_severity_word(&entry.score) {
                return word;
            }
        }
    }
    for value in [
        affected.database_specific.as_ref(),
        affected.ecosystem_specific.as_ref(),
        vuln.database_specific.as_ref(),
    ] {
        if let Some(serde_json::Value::Object(map)) = value
            && let Some(serde_json::Value::String(score)) = map.get("severity")
            && let Some(word) = known_severity_word(score)
        {
            return word;
        }
    }
    String::new()
}

fn osv_kind(affected: &osv::schema::Affected) -> AdvisoryKind {
    let word = match affected.database_specific.as_ref() {
        Some(serde_json::Value::Object(map)) => match map.get("informational") {
            Some(serde_json::Value::String(word)) => word.trim().to_ascii_lowercase(),
            _ => return AdvisoryKind::Vulnerability,
        },
        _ => return AdvisoryKind::Vulnerability,
    };
    match word.as_str() {
        "unmaintained" => AdvisoryKind::Unmaintained,
        "unsound" => AdvisoryKind::Unsound,
        _ => AdvisoryKind::Notice,
    }
}

pub(crate) fn range_events_to_intervals(
    events: &[osv::schema::Event],
) -> Vec<(Option<String>, Option<String>, bool)> {
    let mut out: Vec<(Option<String>, Option<String>, bool)> = Vec::new();
    let mut open: Option<Option<String>> = None;
    for event in events {
        match event {
            osv::schema::Event::Introduced(version) => {
                if let Some(prev) = open.take() {
                    out.push((prev, None, false));
                }
                let trimmed = version.trim();
                if trimmed == "0" || trimmed.is_empty() {
                    open = Some(None);
                } else {
                    open = Some(Some(trimmed.to_owned()));
                }
            }
            osv::schema::Event::Fixed(version) => {
                let upper = version.trim().to_owned();
                if let Some(lower) = open.take() {
                    out.push((lower, Some(upper), false));
                } else {
                    out.push((None, Some(upper), false));
                }
            }
            osv::schema::Event::LastAffected(version) => {
                let upper = version.trim().to_owned();
                if let Some(lower) = open.take() {
                    out.push((lower, Some(upper), true));
                } else {
                    out.push((None, Some(upper), true));
                }
            }
            osv::schema::Event::Limit(version) => {
                let upper = version.trim().to_owned();
                if let Some(lower) = open.take() {
                    out.push((lower, Some(upper), false));
                } else {
                    out.push((None, Some(upper), false));
                }
            }
            _ => {}
        }
    }
    if let Some(lower) = open.take() {
        out.push((lower, None, false));
    }
    out
}

fn range_scope(lower: Option<&str>, upper: Option<&str>, upper_inclusive: bool) -> Option<String> {
    match (lower, upper) {
        (None, None) => Some("*".to_owned()),
        (None, Some(upper)) => {
            if upper_inclusive {
                Some(format!("<={upper}"))
            } else {
                Some(format!("<{upper}"))
            }
        }
        (Some(lower), None) => Some(format!(">={lower}")),
        (Some(lower), Some(upper)) => {
            if upper_inclusive {
                Some(format!(">={lower}, <={upper}"))
            } else {
                Some(format!(">={lower}, <{upper}"))
            }
        }
    }
}

pub(crate) fn interval_to_scope(
    set: &str,
    lower: &Option<String>,
    upper: &Option<String>,
    upper_inclusive: bool,
) -> Option<String> {
    let lower = lower
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    let upper = upper
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());
    match set {
        "cargo" | "npm" | "go" | "ruby" | "uv" | "cpan" => {
            range_scope(lower, upper, upper_inclusive)
        }
        _ if is_version_family(set) => range_scope(lower, upper, upper_inclusive),
        "maven" | "nuget" => match (lower, upper) {
            (None, None) => Some("[0,)".to_owned()),
            (None, Some(upper)) => {
                if upper_inclusive {
                    Some(format!("(,{upper}]"))
                } else {
                    Some(format!("(,{upper})"))
                }
            }
            (Some(lower), None) => Some(format!("[{lower},)")),
            (Some(lower), Some(upper)) => {
                if upper_inclusive {
                    Some(format!("[{lower},{upper}]"))
                } else {
                    Some(format!("[{lower},{upper})"))
                }
            }
        },
        _ => None,
    }
}

fn project_osv_affected(
    vuln: &osv::schema::Vulnerability,
    affected: &osv::schema::Affected,
) -> Vec<Advisory> {
    let id = vuln.id.trim();
    if id.is_empty() {
        return Vec::new();
    }
    let package = match affected.package.as_ref() {
        Some(package) => package,
        None => return Vec::new(),
    };
    let name = package.name.trim();
    if name.is_empty() {
        return Vec::new();
    }
    let set = match ecosystem_to_set(&package.ecosystem) {
        Some(set) => set,
        None => return Vec::new(),
    };
    let severity = osv_severity_text(vuln, affected);
    let kind = osv_kind(affected);
    let mut fixed: Vec<String> = Vec::new();
    if let Some(ranges) = affected.ranges.as_ref() {
        for range in ranges {
            if matches!(&range.range_type, osv::schema::RangeType::Git) {
                continue;
            }
            for event in &range.events {
                if let osv::schema::Event::Fixed(version) = event {
                    let trimmed = version.trim();
                    if !trimmed.is_empty() && !fixed.iter().any(|seen| seen == trimmed) {
                        fixed.push(trimmed.to_owned());
                    }
                }
            }
        }
    }
    let mut scopes: Vec<String> = Vec::new();
    if let Some(versions) = affected.versions.as_ref() {
        for version in versions {
            let trimmed = version.trim();
            if trimmed.is_empty() {
                continue;
            }
            let exact_prefix = matches!(set, "cargo" | "go" | "uv") || is_version_family(set);
            let scope = if exact_prefix && !trimmed.starts_with('=') {
                format!("={trimmed}")
            } else {
                trimmed.to_owned()
            };
            if !scopes.iter().any(|seen| seen == &scope) {
                scopes.push(scope);
            }
        }
    }
    if let Some(ranges) = affected.ranges.as_ref() {
        for range in ranges {
            if matches!(&range.range_type, osv::schema::RangeType::Git) {
                continue;
            }
            for (lower, upper, inclusive) in range_events_to_intervals(&range.events) {
                if let Some(scope) = interval_to_scope(set, &lower, &upper, inclusive)
                    && !scope.trim().is_empty()
                    && !scopes.iter().any(|seen| seen == &scope)
                {
                    scopes.push(scope);
                }
            }
        }
    }
    scopes
        .into_iter()
        .map(|versions| Advisory {
            id: id.to_owned(),
            package: name.to_owned(),
            versions,
            severity: severity.clone(),
            fixed: fixed.clone(),
            set: set.to_owned(),
            kind,
        })
        .collect()
}

pub(crate) fn project_osv_snapshot(vulns: &[osv::schema::Vulnerability]) -> Vec<Advisory> {
    let mut out = Vec::new();
    for vuln in vulns {
        if vuln.withdrawn.is_some() {
            continue;
        }
        if let Some(entries) = vuln.affected.as_ref() {
            for affected in entries {
                out.extend(project_osv_affected(vuln, affected));
            }
        }
    }
    out
}
