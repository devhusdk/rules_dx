use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::vuln::VersionScope;

pub const EXCEPTION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskException {
    pub advisory: String,
    pub package: String,
    pub set: String,
    pub versions: String,
    pub reason: String,
    pub expires: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UncoveredAcceptance {
    pub set: String,
    pub reason: String,
    pub expires: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SecurityPolicy {
    pub exceptions: Vec<RiskException>,
    pub uncovered: Vec<UncoveredAcceptance>,
}

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

pub fn load_security_toml(text: &str) -> Result<SecurityPolicy, SecurityProblem> {
    let file: SecurityFile =
        toml::from_str(text).map_err(|error| SecurityProblem::InvalidToml {
            message: error.to_string(),
        })?;
    let version = file.schema_version.unwrap_or(EXCEPTION_SCHEMA_VERSION);
    if version != EXCEPTION_SCHEMA_VERSION {
        return Err(SecurityProblem::UnsupportedSchema { version });
    }
    Ok(SecurityPolicy {
        exceptions: file.exception,
        uncovered: file.uncovered,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityFile {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    exception: Vec<RiskException>,
    #[serde(default)]
    uncovered: Vec<UncoveredAcceptance>,
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
