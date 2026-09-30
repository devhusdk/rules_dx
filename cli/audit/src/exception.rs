use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SecurityPolicy {
    pub exceptions: Vec<RiskException>,
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

pub fn version_in_scope(scope: &str, version: &str) -> bool {
    let requirements = match semver::VersionReq::parse(scope) {
        Ok(requirements) => requirements,
        Err(_) => return false,
    };
    let version = match semver::Version::parse(version) {
        Ok(version) => version,
        Err(_) => return false,
    };
    requirements.matches(&version)
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
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityFile {
    #[serde(default)]
    schema_version: Option<u32>,
    #[serde(default)]
    exception: Vec<RiskException>,
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
