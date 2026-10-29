use crate::vuln::VersionScope;
use pep440_rs::Version;
use std::str::FromStr;

pub fn normalize_pypi_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut separator = false;
    for ch in name.trim().chars() {
        if matches!(ch, '-' | '_' | '.') {
            separator = true;
            continue;
        }
        if separator && !out.is_empty() {
            out.push('-');
        }
        separator = false;
        out.extend(ch.to_lowercase());
    }
    out
}

fn clause(part: &str) -> Option<(&str, Version)> {
    let part = part.trim();
    for op in [">=", "<=", "!=", "==", ">", "<", "="] {
        if let Some(rest) = part.strip_prefix(op) {
            let version = Version::from_str(rest.trim()).ok()?;
            return Some((op, version));
        }
    }
    None
}

fn single_exact_clause(scope: &str) -> bool {
    !scope.contains(',') && (scope.starts_with("==") || scope.starts_with('='))
}

pub fn pypi_assess(scope: &str, version: &str) -> VersionScope {
    let scope = scope.trim();
    if scope.is_empty() {
        return VersionScope::Unaffected;
    }
    let Ok(locked) = Version::from_str(version.trim()) else {
        return VersionScope::Incomplete;
    };
    if scope == "*" {
        return VersionScope::Affected;
    }
    let mut affected = true;
    for part in scope.split(',') {
        let Some((op, bound)) = clause(part) else {
            // OSV lists legacy tags such as `v0.1.1d` that are not PEP 440 versions. A locked
            // version always parses, so an exact match against one can never hold.
            if single_exact_clause(scope) {
                return VersionScope::Unaffected;
            }
            return VersionScope::Incomplete;
        };
        let holds = match op {
            ">=" => locked >= bound,
            "<=" => locked <= bound,
            "!=" => locked != bound,
            ">" => locked > bound,
            "<" => locked < bound,
            _ => locked == bound,
        };
        affected &= holds;
    }
    if affected {
        VersionScope::Affected
    } else {
        VersionScope::Unaffected
    }
}

pub fn pypi_in_scope(scope: &str, version: &str) -> bool {
    pypi_assess(scope, version) == VersionScope::Affected
}
