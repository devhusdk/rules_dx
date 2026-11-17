//! Version satisfaction, one grammar per ecosystem.

use dx_semver::Version;

use crate::ecosystem::python;
use dx_ecosystems::Language;

/// Settles `spec` against `locked` in the grammar `eco` declares requirements in.
#[must_use]
pub fn satisfies_for(eco: Language, spec: &str, locked: &str) -> bool {
    if eco == Language::Python
        && let Some(matched) = python::satisfies_py(spec, locked)
    {
        return matched;
    }
    if eco.uses_semver_grammar() {
        let spec = if eco == Language::Dart {
            dart_range(spec)
        } else {
            spec.to_owned()
        };
        if let Some(matched) = satisfies_semver(&spec, locked) {
            return matched;
        }
    }
    satisfies_non_semver(spec, locked)
}

/// Dart separates the bounds of a range with spaces (`>=1.0.0 <2.0.0`), semver with commas.
fn dart_range(spec: &str) -> String {
    let mut out = String::new();
    for word in spec.split_whitespace() {
        let bound = word.starts_with(['<', '>', '=']);
        if !out.is_empty() && bound {
            out.push_str(", ");
        } else if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

/// Settles a semver requirement, or `None` when either side is outside the semver grammar.
fn satisfies_semver(spec: &str, locked: &str) -> Option<bool> {
    let requirement = semver::VersionReq::parse(normalize(spec).as_str()).ok()?;
    let version = semver::Version::parse(strip_v_prefix(locked.trim()).as_str()).ok()?;
    Some(requirement.matches(&version))
}

fn strip_v_prefix(value: &str) -> String {
    match value.strip_prefix('v') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest.to_owned(),
        _ => value.to_owned(),
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

/// Reads a version as Maven, `RubyGems` and Homebrew write it: numeric parts split by `.` or `-`
/// (`1.2.3-1` is `1.2.3.1`), then a suffix semver may not accept. A suffix that starts with a
/// prerelease marker makes a prerelease of that marker; any other, like `-jre`, `.Final`, `_292`
/// or a wildcard `.x`, names a flavor of the release, so it compares as the release.
fn version_of(value: &str) -> Option<Version> {
    let base = value.split('+').next().unwrap_or(value);
    let mut numbers = Vec::new();
    let mut suffix = "";
    for segment in base.split(['.', '-']) {
        let tail = segment.trim_start_matches(|c: char| c.is_ascii_digit());
        let digits = segment.strip_suffix(tail).unwrap_or_default();
        if !digits.is_empty() {
            numbers.push(digits);
        }
        if digits.is_empty() || !tail.is_empty() {
            suffix = tail;
            break;
        }
    }
    let release = numbers.join(".");
    let suffix = suffix.to_ascii_lowercase();
    // The canonical text is plain numbers plus a bare marker, which `Version::parse` always reads.
    match PRERELEASE_MARKERS
        .iter()
        .find(|marker| suffix.starts_with(**marker))
    {
        Some(marker) => Version::parse(&format!("{release}-{marker}")).ok(),
        None => Version::parse(&release).ok(),
    }
}

fn satisfies_range(spec: &str, locked: &Version) -> Option<bool> {
    for part in spec.split(',') {
        let part = part.trim();
        let (op, rest) = ["<=", ">=", "!=", "<", ">"]
            .iter()
            .find_map(|op| part.strip_prefix(op).map(|rest| (*op, rest)))?;
        let want = version_of(rest.trim())?;
        let holds = match op {
            ">=" => *locked >= want,
            "<=" => *locked <= want,
            ">" => *locked > want,
            "!=" => *locked != want,
            _ => *locked < want,
        };
        if !holds {
            return Some(false);
        }
    }
    Some(true)
}

/// `~> 1.2.3` lets only the last written part rise; `~1.2.3` lets the patch rise.
fn satisfies_tilde(spec: &str, locked: &Version) -> Option<bool> {
    let (pessimistic, rest) = match spec.strip_prefix('>') {
        Some(rest) => (true, rest),
        None => (false, spec),
    };
    let want = version_of(rest.trim())?;
    let keep = match (want.parts(), pessimistic) {
        (0 | 1, _) => 1,
        (len, true) => len - 1,
        (_, false) => 2,
    };
    Some(*locked >= want && locked.same_prefix(&want, keep))
}

/// Whether two exact versions are the same, so `4.0` equals `4.0.0`; the prerelease and build
/// must match as written.
pub(crate) fn versions_equal(spec: &str, locked: &str) -> bool {
    match (Version::parse(spec), Version::parse(locked)) {
        (Ok(left), Ok(right)) => left == right && left.build() == right.build(),
        (Ok(_) | Err(_), _) => spec == locked,
    }
}

/// Whether `text` pins a version: it writes at least three numeric parts, whatever follows.
fn is_full_version(text: &str) -> bool {
    version_of(text).is_some_and(|version| version.parts() >= 3)
}

/// Settles the grammars semver does not model: Maven, `NuGet`, `RubyGems`, PEP 440.
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
        return version_of(&locked_norm)
            .and_then(|locked| satisfies_tilde(&rest, &locked))
            .unwrap_or(rest == locked_norm);
    } else if s.starts_with(['<', '>', '!']) {
        return version_of(&locked_norm)
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
