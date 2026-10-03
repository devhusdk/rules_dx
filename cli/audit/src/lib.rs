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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRequest {
    pub families: Vec<AuditFamily>,
    pub scopes: Vec<String>,
}

impl AuditRequest {
    pub fn effective_scopes(&self) -> Vec<String> {
        if self.scopes.is_empty() {
            vec![DEFAULT_SCOPE.to_owned()]
        } else {
            self.scopes.clone()
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
    fn default_scope_applies_only_to_an_empty_scope_list() {
        let bare = AuditRequest {
            families: vec![AuditFamily::Security],
            scopes: Vec::new(),
        };
        assert_eq!(bare.effective_scopes(), vec!["//...".to_owned()]);
        let scoped = AuditRequest {
            families: vec![AuditFamily::Security],
            scopes: vec!["//services/payments/...".to_owned()],
        };
        assert_eq!(
            scoped.effective_scopes(),
            vec!["//services/payments/...".to_owned()]
        );
    }
}
