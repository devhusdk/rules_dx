#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod advisory;
pub mod advisory_prep;
pub mod backend;
pub mod curator;
pub mod exception;
pub mod license_expr;
pub mod license_notice;
pub mod license_policy;
pub mod locks;
pub mod outcome;
pub mod secrets;
pub mod spdx;
pub mod vuln;

pub const SECURITY_FAMILY: &str = "security";
pub const LICENSE_FAMILY: &str = "license";

pub const DEFAULT_SCOPE: &str = "//...";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditFamily {
    Security,
    License,
}

impl AuditFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            AuditFamily::Security => SECURITY_FAMILY,
            AuditFamily::License => LICENSE_FAMILY,
        }
    }
}

/// Which targets an audit covers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuditScope {
    /// No scope was given, so the audit covers the whole workspace.
    Default,
    /// Only these scopes; an empty list audits no dependency set.
    Only(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRequest {
    pub families: Vec<AuditFamily>,
    pub scope: AuditScope,
}

impl AuditRequest {
    /// Returns the scopes to audit.
    pub fn effective_scopes(&self) -> Vec<String> {
        match &self.scope {
            AuditScope::Default => vec![DEFAULT_SCOPE.to_owned()],
            AuditScope::Only(scopes) => scopes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_spellings_are_frozen() {
        assert_eq!(AuditFamily::Security.as_str(), "security");
        assert_eq!(AuditFamily::License.as_str(), "license");
    }

    #[test]
    fn default_scope_applies_only_without_an_explicit_scope() {
        let bare = AuditRequest {
            families: vec![AuditFamily::Security],
            scope: AuditScope::Default,
        };
        assert_eq!(bare.effective_scopes(), vec!["//...".to_owned()]);
        let scoped = AuditRequest {
            families: vec![AuditFamily::Security],
            scope: AuditScope::Only(vec!["//services/payments/...".to_owned()]),
        };
        assert_eq!(
            scoped.effective_scopes(),
            vec!["//services/payments/...".to_owned()]
        );
        let nothing = AuditRequest {
            families: vec![AuditFamily::Security],
            scope: AuditScope::Only(Vec::new()),
        };
        assert!(nothing.effective_scopes().is_empty());
    }
}
