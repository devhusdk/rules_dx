use pep440_rs::Version;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PypiScope {
    Affected,
    Unaffected,
    Incomplete,
}

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

pub fn pypi_assess(scope: &str, version: &str) -> PypiScope {
    let scope = scope.trim();
    if scope.is_empty() {
        return PypiScope::Unaffected;
    }
    let Ok(locked) = Version::from_str(version.trim()) else {
        return PypiScope::Incomplete;
    };
    if scope == "*" {
        return PypiScope::Affected;
    }
    let mut affected = true;
    for part in scope.split(',') {
        let Some((op, bound)) = clause(part) else {
            return PypiScope::Incomplete;
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
        PypiScope::Affected
    } else {
        PypiScope::Unaffected
    }
}

pub fn pypi_in_scope(scope: &str, version: &str) -> bool {
    pypi_assess(scope, version) == PypiScope::Affected
}
