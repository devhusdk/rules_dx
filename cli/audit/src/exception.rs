use chrono::{Datelike, NaiveDate};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::vuln::{AdvisoryKind, VersionScope};

pub const EXCEPTION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RiskException {
    pub advisory: String,
    pub package: String,
    pub set: String,
    pub versions: String,
    pub reason: String,
    pub expires: String,
    #[serde(skip)]
    pub line: usize,
}

impl RiskException {
    pub fn entry_label(&self) -> String {
        format!(
            "{} on {}",
            or_unknown(&self.advisory),
            or_unknown(&self.package)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UncoveredAcceptance {
    pub set: String,
    pub reason: String,
    pub expires: String,
    #[serde(skip)]
    pub line: usize,
}

impl UncoveredAcceptance {
    pub fn entry_label(&self) -> String {
        or_unknown(&self.set).to_owned()
    }
}

pub fn or_unknown(value: &str) -> &str {
    if value.trim().is_empty() {
        "?"
    } else {
        value
    }
}

pub fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    1 + text.as_bytes()[..end]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityPolicy {
    pub exceptions: Vec<RiskException>,
    pub uncovered: Vec<UncoveredAcceptance>,
    pub fail_on: Vec<AdvisoryKind>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            exceptions: Vec::new(),
            uncovered: Vec::new(),
            fail_on: STANDARD_FAIL_ON.to_vec(),
        }
    }
}

impl SecurityPolicy {
    pub fn fails_on(&self, kind: AdvisoryKind) -> bool {
        self.fail_on.contains(&kind)
    }
}

const STANDARD_FAIL_ON: [AdvisoryKind; 2] = [AdvisoryKind::Vulnerability, AdvisoryKind::Unsound];

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SecurityProblem {
    #[error("security.toml is invalid: {message}")]
    InvalidToml { message: String },
    #[error(
        "unsupported security.toml schema_version {version} (want {EXCEPTION_SCHEMA_VERSION})"
    )]
    UnsupportedSchema { version: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindingRef {
    pub advisory: String,
    pub package: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ExceptionProblem {
    #[error("risk exception missing {field}")]
    MissingField { field: &'static str },
    #[error("uncovered acceptance missing {field}")]
    MissingUncoveredField { field: &'static str },
    #[error("uncovered acceptance for {set} has invalid expiration {value:?}; want YYYY-MM-DD")]
    UncoveredInvalidDate { set: String, value: String },
    #[error("uncovered acceptance for {set} expired {expires} (audit date {today}); renewal needs review")]
    UncoveredExpired {
        set: String,
        expires: String,
        today: String,
    },
    #[error("risk exception has invalid expiration {value:?}; want YYYY-MM-DD")]
    InvalidDate { value: String },
    #[error("risk exception expired {expires} (audit date {today}); renewal needs review")]
    Expired { expires: String, today: String },
    #[error("risk exception for {advisory} on {package} matches no finding; remove it explicitly")]
    Obsolete { advisory: String, package: String },
}

pub fn validate_exception(exception: &RiskException, today: &str) -> Result<(), ExceptionProblem> {
    for (field, value) in [
        ("advisory", exception.advisory.as_str()),
        ("package", exception.package.as_str()),
        ("set", exception.set.as_str()),
        ("versions", exception.versions.as_str()),
        ("reason", exception.reason.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ExceptionProblem::MissingField { field });
        }
    }
    check_expiry(&exception.expires, today)
}

/// The exceptions in `set` that are complete and not yet expired on `today`.
pub fn live_exceptions_for_set<'a>(
    policy: &'a SecurityPolicy,
    set: &str,
    today: &str,
) -> Vec<&'a RiskException> {
    policy
        .exceptions
        .iter()
        .filter(|exception| exception.set == set && validate_exception(exception, today).is_ok())
        .collect()
}

pub fn validate_uncovered(
    acceptance: &UncoveredAcceptance,
    today: &str,
) -> Result<(), ExceptionProblem> {
    for (field, value) in [
        ("set", acceptance.set.as_str()),
        ("reason", acceptance.reason.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ExceptionProblem::MissingUncoveredField { field });
        }
    }
    check_expiry(&acceptance.expires, today).map_err(|problem| match problem {
        ExceptionProblem::Expired { expires, today } => ExceptionProblem::UncoveredExpired {
            set: acceptance.set.clone(),
            expires,
            today,
        },
        ExceptionProblem::InvalidDate { value } => ExceptionProblem::UncoveredInvalidDate {
            set: acceptance.set.clone(),
            value,
        },
        other => other,
    })
}

pub fn check_expiry(expires: &str, today: &str) -> Result<(), ExceptionProblem> {
    let expires_date = parse_audit_date(expires)?;
    let today_date = parse_audit_date(today)?;
    if expires_date <= today_date {
        return Err(ExceptionProblem::Expired {
            expires: expires.to_owned(),
            today: today.to_owned(),
        });
    }
    Ok(())
}

pub fn cargo_assess(scope: &str, version: &str) -> VersionScope {
    if scope.trim().is_empty() {
        return VersionScope::Unaffected;
    }
    let Ok(requirements) = semver::VersionReq::parse(scope) else {
        return VersionScope::Incomplete;
    };
    let Ok(version) = semver::Version::parse(version) else {
        return VersionScope::Incomplete;
    };
    if requirements.matches(&version) {
        VersionScope::Affected
    } else {
        VersionScope::Unaffected
    }
}

pub fn version_in_scope(scope: &str, version: &str) -> bool {
    cargo_assess(scope, version) == VersionScope::Affected
}

fn parse_audit_date(value: &str) -> Result<NaiveDate, ExceptionProblem> {
    if !is_date_shape(value) {
        return Err(ExceptionProblem::InvalidDate {
            value: value.to_owned(),
        });
    }
    match NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(date) if date.year() >= 1 => Ok(date),
        _ => Err(ExceptionProblem::InvalidDate {
            value: value.to_owned(),
        }),
    }
}

fn is_date_shape(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn parse_file(text: &str) -> Result<SecurityFile, SecurityProblem> {
    let file: SecurityFile =
        toml::from_str(text).map_err(|error| SecurityProblem::InvalidToml {
            message: match error.span() {
                Some(span) => format!(
                    "invalid TOML at line {}: {}",
                    line_of(text, span.start),
                    error.message()
                ),
                None => error.to_string(),
            },
        })?;
    let version = file.schema_version.unwrap_or(EXCEPTION_SCHEMA_VERSION);
    if version != EXCEPTION_SCHEMA_VERSION {
        return Err(SecurityProblem::UnsupportedSchema { version });
    }
    Ok(file)
}

/// Parses one file. An `extends` line is accepted and not followed.
pub fn load_security_toml(text: &str) -> Result<SecurityPolicy, SecurityProblem> {
    Ok(build_policy(text, parse_file(text)?, None))
}

/// The policy of the standard base, used when a workspace has no security.toml.
pub fn standard_security_policy() -> Result<SecurityPolicy, SecurityProblem> {
    let text = crate::profile_base::base_text("security", crate::profile_base::Profile::Standard)
        .ok_or_else(|| SecurityProblem::InvalidToml {
        message: "the standard base is missing".to_owned(),
    })?;
    load_security_toml(text)
}

/// Parses `root_text` and merges it over the base named by its `extends`.
/// `read` returns the text of a workspace-relative path. Without `extends` the
/// standard base's `fail_on` applies.
pub fn load_security(
    root_text: &str,
    read: impl Fn(&str) -> std::io::Result<String>,
) -> Result<SecurityPolicy, SecurityProblem> {
    let root = parse_file(root_text)?;
    let Some(path) = root.extends.clone() else {
        return Ok(build_policy(root_text, root, None));
    };
    let invalid = |message: String| SecurityProblem::InvalidToml { message };
    let inside = !path.is_empty()
        && std::path::Path::new(&path)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
        && !path.contains('\\');
    if !inside {
        return Err(invalid(format!(
            "extends {path}: must be a relative path inside the workspace"
        )));
    }
    let base_text = read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            invalid(format!("extends {path}: not found; run dx init --apply"))
        } else {
            invalid(format!("extends {path}: {error}"))
        }
    })?;
    let base =
        parse_file(&base_text).map_err(|error| invalid(format!("extends {path}: {error}")))?;
    if base.extends.is_some() || !base.exception.is_empty() || !base.uncovered.is_empty() {
        return Err(invalid(format!(
            "extends {path}: a base may not have extends, [[exception]] or [[uncovered]]"
        )));
    }
    Ok(build_policy(root_text, root, Some(base)))
}

fn build_policy(text: &str, file: SecurityFile, base: Option<SecurityFile>) -> SecurityPolicy {
    let fail_on = file
        .fail_on
        .or_else(|| base.and_then(|base| base.fail_on))
        .unwrap_or_else(|| STANDARD_FAIL_ON.to_vec());
    SecurityPolicy {
        exceptions: file
            .exception
            .into_iter()
            .map(|entry| {
                let line = line_of(text, entry.span().start);
                RiskException {
                    line,
                    ..entry.into_inner()
                }
            })
            .collect(),
        uncovered: file
            .uncovered
            .into_iter()
            .map(|entry| {
                let line = line_of(text, entry.span().start);
                UncoveredAcceptance {
                    line,
                    ..entry.into_inner()
                }
            })
            .collect(),
        fail_on,
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SecurityFile {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    #[schemars(with = "Vec<RiskException>")]
    exception: Vec<toml::Spanned<RiskException>>,
    #[serde(default)]
    #[schemars(with = "Vec<UncoveredAcceptance>")]
    uncovered: Vec<toml::Spanned<UncoveredAcceptance>>,
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    fail_on: Option<Vec<AdvisoryKind>>,
}

#[cfg(test)]
impl RiskException {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            advisory,
            package,
            set,
            versions,
            reason,
            expires,
            line
        })
        .into_iter()
        .filter(|key| *key != "line")
        .collect()
    }
}

#[cfg(test)]
impl UncoveredAcceptance {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            set,
            reason,
            expires,
            line
        })
        .into_iter()
        .filter(|key| *key != "line")
        .collect()
    }
}

#[cfg(test)]
impl SecurityFile {
    pub(crate) fn key_names() -> Vec<&'static str> {
        key_names!(Self {
            schema_version,
            exception,
            uncovered,
            extends,
            fail_on
        })
    }
}

#[path = "exception_apply.rs"]
mod exception_apply;
#[path = "exception_npm.rs"]
mod exception_npm;

pub use exception_apply::*;
pub use exception_npm::*;

#[cfg(test)]
#[path = "exception_tests.rs"]
mod exception_tests;
