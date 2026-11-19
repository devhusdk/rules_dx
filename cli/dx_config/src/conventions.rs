//! Rules every dx config file follows: a required `schema_version`, label patterns,
//! and exceptions that carry a `reason` and a future `expires` date.

use chrono::{Datelike, NaiveDate};
use dx_path::label::Label;

/// The schema version this dx writes and reads.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The additive revision of the dx.toml schema within `CURRENT_SCHEMA_VERSION`. Adding a key bumps it; files keep
/// declaring only the major.
pub const SCHEMA_MINOR: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ConventionProblem {
    #[error("schema_version is missing; add `schema_version = {CURRENT_SCHEMA_VERSION}`")]
    MissingSchemaVersion,
    #[error("schema_version must be a whole number, found {found}")]
    BadSchemaVersion { found: String },
    #[error(
        "schema_version {found} is not accepted; the only accepted value is {CURRENT_SCHEMA_VERSION}"
    )]
    UnsupportedSchemaVersion { found: i64 },
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("invalid date {value:?}; want YYYY-MM-DD")]
    InvalidDate { value: String },
    #[error("expired {expires} (today is {today}); renewal needs review")]
    Expired { expires: String, today: String },
    #[error("invalid label pattern {value:?}")]
    BadPattern { value: String },
}

/// Checks the `schema_version` value of a parsed file. Only the current version is accepted.
pub fn check_schema_version(value: Option<&toml::Value>) -> Result<(), ConventionProblem> {
    let Some(value) = value else {
        return Err(ConventionProblem::MissingSchemaVersion);
    };
    let Some(found) = value.as_integer() else {
        return Err(ConventionProblem::BadSchemaVersion {
            found: value.to_string(),
        });
    };
    if found != i64::from(CURRENT_SCHEMA_VERSION) {
        return Err(ConventionProblem::UnsupportedSchemaVersion { found });
    }
    Ok(())
}

/// Parses `YYYY-MM-DD`.
pub fn parse_date(value: &str) -> Result<NaiveDate, ConventionProblem> {
    let bytes = value.as_bytes();
    let shaped = bytes.len() == 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit());
    match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(date) if shaped && date.year() >= 1 => Ok(date),
        _ => Err(ConventionProblem::InvalidDate {
            value: value.to_owned(),
        }),
    }
}

/// `expires` must be a valid date after `today`.
pub fn check_expiry(expires: &str, today: &str) -> Result<(), ConventionProblem> {
    let expires_date = parse_date(expires)?;
    let today_date = parse_date(today)?;
    if expires_date <= today_date {
        return Err(ConventionProblem::Expired {
            expires: expires.to_owned(),
            today: today.to_owned(),
        });
    }
    Ok(())
}

/// An exception needs a non-empty `reason` and a future `expires`.
pub fn check_exception(reason: &str, expires: &str, today: &str) -> Result<(), ConventionProblem> {
    if reason.trim().is_empty() {
        return Err(ConventionProblem::EmptyField { field: "reason" });
    }
    check_expiry(expires, today)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PatternKind {
    Recursive,
    Package,
    Target,
}

struct ParsedPattern<'a> {
    repo: &'a str,
    package: &'a str,
    name: &'a str,
    kind: PatternKind,
}

/// A package path is `/`-separated names with no empty, `.` or `..` segment.
fn is_clean_package(package: &str) -> bool {
    package.is_empty()
        || package
            .split('/')
            .all(|part| !matches!(part, "" | "." | ".."))
}

fn parse_pattern(text: &str) -> Option<ParsedPattern<'_>> {
    if text
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return None;
    }
    let label = Label::parse(text)?;
    if label.relative {
        return None;
    }
    // `@r//` and `@@r//` name the same repository here, and `@//` the main one.
    let repo = label.repo_name().unwrap_or_default();
    let (package, name) = (label.package, label.name);
    let all = matches!(name, "all" | "*" | "all-targets");
    if package == "..." || package.ends_with("/...") {
        // `//p/...` leaves the name implicit, which the parser spells `...`.
        if name != "..." && !all {
            return None;
        }
        let package = package
            .strip_suffix("...")
            .unwrap_or(package)
            .trim_end_matches('/');
        if !is_clean_package(package) {
            return None;
        }
        return Some(ParsedPattern {
            repo,
            package,
            name: "",
            kind: PatternKind::Recursive,
        });
    }
    if package.contains("...") || !is_clean_package(package) {
        return None;
    }
    let kind = if all {
        PatternKind::Package
    } else {
        PatternKind::Target
    };
    Some(ParsedPattern {
        repo,
        package,
        name,
        kind,
    })
}

/// Whether the label pattern `entry` covers the scope `root`.
#[must_use]
pub fn pattern_covers(entry: &str, root: &str) -> bool {
    let (Some(entry), Some(root)) = (parse_pattern(entry), parse_pattern(root)) else {
        return false;
    };
    if entry.repo != root.repo {
        return false;
    }
    match entry.kind {
        PatternKind::Recursive => {
            entry.package.is_empty()
                || root.package == entry.package
                || root
                    .package
                    .strip_prefix(entry.package)
                    .is_some_and(|tail| tail.starts_with('/'))
        }
        PatternKind::Package => {
            root.kind != PatternKind::Recursive && root.package == entry.package
        }
        PatternKind::Target => {
            root.kind == PatternKind::Target
                && root.package == entry.package
                && root.name == entry.name
        }
    }
}

/// The workspace-relative directory of a recursive pattern in the main repository, such as
/// `vendor` for `//vendor/...:all`. `//...` and external patterns have none.
#[must_use]
pub fn recursive_dir(text: &str) -> Option<&str> {
    let pattern = parse_pattern(text)?;
    (pattern.kind == PatternKind::Recursive
        && pattern.repo.is_empty()
        && !pattern.package.is_empty())
    .then_some(pattern.package)
}

/// Whether `text` is a Bazel label pattern dx understands.
#[must_use]
pub fn is_valid_pattern(text: &str) -> bool {
    parse_pattern(text).is_some()
}
