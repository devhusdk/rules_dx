//! Version satisfaction, one grammar per ecosystem.

use std::cmp::Ordering;

use semver::{Version, VersionReq};

use crate::ecosystem::python;
use crate::Ecosystem;

/// Settles `spec` against `locked` in the grammar `eco` declares requirements in.
pub fn satisfies_for(eco: Ecosystem, spec: &str, locked: &str) -> bool {
    if eco == Ecosystem::Python {
        if let Some(matched) = python::satisfies_py(spec, locked) {
            return matched;
        }
    }
    if eco.uses_semver_grammar() {
        if let Some(matched) = satisfies_semver(spec, locked) {
            return matched;
        }
    }
    satisfies_non_semver(spec, locked)
}

/// Settles a semver requirement, or `None` when either side is outside the semver grammar.
fn satisfies_semver(spec: &str, locked: &str) -> Option<bool> {
    let requirement = VersionReq::parse(normalize(spec).as_str()).ok()?;
    let version = Version::parse(strip_v_prefix(locked.trim()).as_str()).ok()?;
    Some(requirement.matches(&version))
}

fn strip_v_prefix(value: &str) -> String {
    if value.len() > 1
        && value.starts_with('v')
        && value[1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        value[1..].to_owned()
    } else {
        value.to_owned()
    }
}

fn normalize(spec: &str) -> String {
    let mut text = spec
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_owned();
    if let Some(head) = text.split(';').next() {
        text = head.trim().to_owned();
    }
    strip_v_prefix(&text)
}

const PRERELEASE_MARKERS: [&str; 9] = [
    "alpha",
    "beta",
    "rc",
    "cr",
    "pre",
    "preview",
    "snapshot",
    "dev",
    "milestone",
];

fn parse_version(value: &str) -> Option<(Vec<u64>, bool)> {
    let base = value.split('+').next().unwrap_or(value);
    let mut numbers = Vec::new();
    let mut suffix = String::new();
    for segment in base.split(['.', '-']) {
        let digits: String = segment.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            numbers.push(digits.parse::<u64>().ok()?);
        }
        suffix = segment[digits.len()..].to_ascii_lowercase();
        if !suffix.is_empty() {
            break;
        }
    }
    if numbers.is_empty() {
        return None;
    }
    let prerelease = PRERELEASE_MARKERS
        .iter()
        .any(|marker| suffix.starts_with(marker));
    Some((numbers, prerelease))
}

fn cmp_versions(a: &(Vec<u64>, bool), b: &(Vec<u64>, bool)) -> Ordering {
    let len = a.0.len().max(b.0.len());
    let pad = |parts: &[u64]| {
        let mut padded = parts.to_vec();
        padded.resize(len, 0);
        padded
    };
    pad(&a.0).cmp(&pad(&b.0)).then_with(|| b.1.cmp(&a.1))
}

fn upper_bound(parts: &[u64], pessimistic: bool) -> (Vec<u64>, bool) {
    let keep = match (parts.len(), pessimistic) {
        (0 | 1, _) => 1,
        (len, true) => len - 1,
        (_, false) => 2,
    };
    let mut bound = parts[..keep].to_vec();
    bound[keep - 1] += 1;
    (bound, true)
}

fn satisfies_range(spec: &str, locked: &(Vec<u64>, bool)) -> Option<bool> {
    for part in spec.split(',') {
        let part = part.trim();
        let (op, rest) = ["<=", ">=", "<", ">"]
            .iter()
            .find_map(|op| part.strip_prefix(op).map(|rest| (*op, rest)))?;
        let want = parse_version(rest.trim())?;
        let ordering = cmp_versions(locked, &want);
        let holds = match op {
            ">=" => ordering != Ordering::Less,
            "<=" => ordering != Ordering::Greater,
            ">" => ordering == Ordering::Greater,
            _ => ordering == Ordering::Less,
        };
        if !holds {
            return Some(false);
        }
    }
    Some(true)
}

fn satisfies_tilde(spec: &str, locked: &(Vec<u64>, bool)) -> Option<bool> {
    let (pessimistic, rest) = match spec.strip_prefix('>') {
        Some(rest) => (true, rest),
        None => (false, spec),
    };
    let want = parse_version(rest.trim())?;
    let upper = upper_bound(&want.0, pessimistic);
    Some(
        cmp_versions(locked, &want) != Ordering::Less
            && cmp_versions(locked, &upper) == Ordering::Less,
    )
}

pub(crate) fn versions_equal(spec: &str, locked: &str) -> bool {
    if spec.contains(['-', '+']) || locked.contains(['-', '+']) {
        return spec == locked;
    }
    fn numeric_parts(value: &str) -> Option<Vec<u64>> {
        let mut parts = Vec::new();
        for part in value.split('.') {
            if part.is_empty() || !part.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            parts.push(part.parse::<u64>().ok()?);
        }
        Some(parts)
    }
    match (numeric_parts(spec), numeric_parts(locked)) {
        (Some(mut a), Some(mut b)) => {
            while a.len() < b.len() {
                a.push(0);
            }
            while b.len() < a.len() {
                b.push(0);
            }
            a == b
        }
        _ => spec == locked,
    }
}

fn is_full_version(text: &str) -> bool {
    let mut parts = text.split(['.', '-']);
    let (Some(a), Some(b), Some(c)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !a.is_empty()
        && !b.is_empty()
        && !c.is_empty()
        && a.chars().all(|c| c.is_ascii_digit())
        && b.chars().all(|c| c.is_ascii_digit())
        && c.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Settles the grammars semver does not model: Maven, NuGet, RubyGems, Conan, PEP 440.
pub(crate) fn satisfies_non_semver(spec: &str, locked: &str) -> bool {
    let mut s = normalize(spec);
    let locked_norm = strip_v_prefix(locked.trim());
    let mut exact = false;
    if let Some(rest) = s.strip_prefix("==") {
        s = rest.trim().to_owned();
        exact = true;
    } else if let Some(rest) = s.strip_prefix('=') {
        s = rest.trim().to_owned();
        exact = true;
    } else if let Some(rest) = s.strip_prefix('^') {
        s = rest.trim().to_owned();
    } else if let Some(rest) = s.strip_prefix('~') {
        let rest = rest.trim().to_owned();
        return parse_version(&locked_norm)
            .and_then(|locked| satisfies_tilde(&rest, &locked))
            .unwrap_or(rest == locked_norm);
    } else if s.starts_with(['<', '>']) {
        return parse_version(&locked_norm)
            .and_then(|locked| satisfies_range(&s, &locked))
            .unwrap_or(s == locked_norm);
    }
    if s == "*" || s.is_empty() {
        return true;
    }
    if exact || is_full_version(&s) {
        return versions_equal(&s, locked_norm.trim());
    }
    let smajor = s.split('.').next().unwrap_or("").trim().to_owned();
    let lmajor = locked_norm
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_owned();
    if !smajor.is_empty() && smajor.chars().all(|c| c.is_ascii_digit()) {
        return smajor == lmajor;
    }
    smajor == lmajor
}
