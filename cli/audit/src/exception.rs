use schemars::JsonSchema;
use serde::Deserialize;

use crate::profile_base::{base_text, Profile};
use crate::vuln::{AdvisoryKind, VersionScope};

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
    use dx_config::conventions::{self, ConventionProblem};
    conventions::check_expiry(expires, today).map_err(|problem| match problem {
        ConventionProblem::Expired { expires, today } => {
            ExceptionProblem::Expired { expires, today }
        }
        ConventionProblem::InvalidDate { value } => ExceptionProblem::InvalidDate { value },
        other => ExceptionProblem::InvalidDate {
            value: other.to_string(),
        },
    })
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

fn parse_file(text: &str) -> Result<SecurityFile, SecurityProblem> {
    let invalid = |message: String| SecurityProblem::InvalidToml { message };
    let table: toml::Table = text.parse().map_err(|error: toml::de::Error| {
        invalid(toml_message(text, &error, error.message().to_owned()))
    })?;
    dx_config::conventions::check_schema_version(table.get("schema_version"))
        .map_err(|problem| invalid(problem.to_string()))?;
    toml::from_str(text).map_err(|error: toml::de::Error| {
        let message = dx_config::conventions::explain_unknown_key(error.message());
        invalid(toml_message(text, &error, message))
    })
}

pub(crate) fn toml_message(text: &str, error: &toml::de::Error, message: String) -> String {
    match error.span() {
        Some(span) => format!(
            "invalid TOML at line {}: {message}",
            line_of(text, span.start)
        ),
        None => message,
    }
}

/// The policy for `profile`: its embedded base with `text`, the workspace's
/// security.toml, merged over it. Missing or blank text means the base alone.
pub fn load_security(
    text: Option<&str>,
    profile: Profile,
) -> Result<SecurityPolicy, SecurityProblem> {
    let base_text = base_text("security", profile).ok_or_else(|| SecurityProblem::InvalidToml {
        message: format!("the {} base is missing", profile.as_str()),
    })?;
    let base = parse_file(base_text)?;
    match text.filter(|text| !text.trim().is_empty()) {
        Some(text) => Ok(build_policy(text, parse_file(text)?, base)),
        None => Ok(build_policy("", SecurityFile::default(), base)),
    }
}

fn build_policy(text: &str, file: SecurityFile, base: SecurityFile) -> SecurityPolicy {
    let fail_on = file
        .fail_on
        .or(base.fail_on)
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

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SecurityFile {
    // Checked by `parse_file` before the rest of the file.
    #[allow(dead_code)]
    schema_version: u32,
    #[serde(default)]
    #[schemars(with = "Vec<RiskException>")]
    exception: Vec<toml::Spanned<RiskException>>,
    #[serde(default)]
    #[schemars(with = "Vec<UncoveredAcceptance>")]
    uncovered: Vec<toml::Spanned<UncoveredAcceptance>>,
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
