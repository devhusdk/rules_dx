//! The audit date and the workspace policy files shared by the audit, quality and update verbs.

use dx_error::DxError;
use std::path::Path;

/// The date advisory freshness and exception windows are judged against, pinned by `DX_AUDIT_TODAY`.
pub const AUDIT_TODAY_ENV: &str = "DX_AUDIT_TODAY";

/// Returns the audit date, preferring a pinned `YYYY-MM-DD` and ignoring anything else.
pub fn today_utc(pinned: Option<&str>) -> String {
    pinned
        .filter(|date| date.len() == 10 && date.bytes().all(|b| b.is_ascii_digit() || b == b'-'))
        .map(str::to_owned)
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("could not read {rel}: {error}")]
    Read {
        rel: String,
        #[source]
        error: std::io::Error,
    },
    #[error("could not read {rel}: not valid UTF-8")]
    NotUtf8 { rel: String },
    #[error(transparent)]
    SecurityInvalid(dx_audit::exception::SecurityProblem),
}

/// The UTF-8 text of the workspace file `rel`.
pub fn read_workspace_text(workspace: &Path, rel: &str) -> Result<String, PolicyError> {
    match dx_path::workspace::resolve_in_workspace(workspace, rel).and_then(std::fs::read) {
        Err(error) => Err(PolicyError::Read {
            rel: rel.to_owned(),
            error,
        }),
        Ok(bytes) => String::from_utf8(bytes)
            .ok()
            .ok_or_else(|| PolicyError::NotUtf8 {
                rel: rel.to_owned(),
            }),
    }
}

/// The tier from the dx.toml `tier`; strict without one.
pub fn audit_tier(workspace: &Path) -> Result<dx_audit::tier_base::Tier, DxError> {
    match dx_config::load(workspace) {
        Ok(config) => Ok(config.and_then(|config| config.tier).unwrap_or_default()),
        Err(problem) => Err(DxError::failure(format!("dx.toml: {problem}"))),
    }
}

/// The text of `rel`, or `None` when the file does not exist.
pub fn read_policy_text(workspace: &Path, rel: &str) -> Result<Option<String>, PolicyError> {
    match read_workspace_text(workspace, rel) {
        Err(PolicyError::Read { error, .. }) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(None)
        }
        other => other.map(Some),
    }
}

/// The `security.toml` exceptions under the workspace tier.
pub fn load_security_policy(
    workspace: &Path,
) -> Result<dx_audit::exception::SecurityPolicy, PolicyError> {
    use dx_audit::exception::{SecurityProblem, load_security};
    let tier = audit_tier(workspace).map_err(|error| {
        PolicyError::SecurityInvalid(SecurityProblem::InvalidToml {
            message: error.message().to_owned(),
        })
    })?;
    let text = read_policy_text(workspace, "security.toml")?;
    load_security(text.as_deref(), tier).map_err(PolicyError::SecurityInvalid)
}

#[cfg(test)]
#[path = "audit_policy_tests.rs"]
mod tests;
