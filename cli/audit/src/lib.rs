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

    pub fn parse(text: &str) -> Option<AuditFamily> {
        match text {
            SECURITY_FAMILY => Some(AuditFamily::Security),
            LICENSE_FAMILY => Some(AuditFamily::License),
            _ => None,
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

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AuditPlanError {
    #[error(
        "unknown audit family {value:?}; want `{SECURITY_FAMILY}`, `{LICENSE_FAMILY}`, or a scope"
    )]
    UnknownFamily { value: String },
}

pub fn plan_audit(args: &[String]) -> Result<AuditRequest, AuditPlanError> {
    let mut families = vec![AuditFamily::Security, AuditFamily::License];
    let mut scopes: &[String] = args;
    if let Some((first, rest)) = args.split_first() {
        if let Some(family) = AuditFamily::parse(first) {
            families = vec![family];
            scopes = rest;
        } else if looks_like_scope(first) {
            scopes = args;
        } else {
            return Err(AuditPlanError::UnknownFamily {
                value: first.clone(),
            });
        }
    }
    Ok(AuditRequest {
        families,
        scopes: scopes.to_vec(),
    })
}

fn looks_like_scope(text: &str) -> bool {
    text.starts_with("//") || text.starts_with('@') || text.starts_with('.') || text.contains('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().copied().map(str::to_owned).collect()
    }

    #[test]
    fn bare_audit_runs_both_families_security_first() {
        let request = plan_audit(&[]).expect("bare audit plans");
        assert_eq!(
            request.families,
            vec![AuditFamily::Security, AuditFamily::License]
        );
        assert!(request.scopes.is_empty());
        assert_eq!(request.effective_scopes(), vec!["//...".to_owned()]);
    }

    #[test]
    fn explicit_families_run_alone() {
        let security = plan_audit(&args(&["security"])).expect("security plans");
        assert_eq!(security.families, vec![AuditFamily::Security]);
        assert_eq!(security.effective_scopes(), vec!["//...".to_owned()]);
        let license = plan_audit(&args(&["license"])).expect("license plans");
        assert_eq!(license.families, vec![AuditFamily::License]);
    }

    #[test]
    fn scopes_pass_through_verbatim_with_both_families() {
        let request =
            plan_audit(&args(&["//services/payments/...", "@crates//:lock"])).expect("plans");
        assert_eq!(
            request.families,
            vec![AuditFamily::Security, AuditFamily::License]
        );
        assert_eq!(
            request.effective_scopes(),
            vec![
                "//services/payments/...".to_owned(),
                "@crates//:lock".to_owned()
            ]
        );
    }

    #[test]
    fn family_plus_scopes_narrows_family_keeps_scopes() {
        let request = plan_audit(&args(&["license", "//services/payments/..."])).expect("plans");
        assert_eq!(request.families, vec![AuditFamily::License]);
        assert_eq!(
            request.effective_scopes(),
            vec!["//services/payments/...".to_owned()]
        );
    }

    #[test]
    fn unknown_first_positional_fails_closed() {
        let error = plan_audit(&args(&["licence"])).expect_err("must reject");
        assert_eq!(
            error,
            AuditPlanError::UnknownFamily {
                value: "licence".to_owned()
            }
        );
    }

    #[test]
    fn family_spellings_are_frozen() {
        assert_eq!(AuditFamily::Security.as_str(), "security");
        assert_eq!(AuditFamily::License.as_str(), "license");
        assert_eq!(AuditFamily::parse("security"), Some(AuditFamily::Security));
        assert_eq!(AuditFamily::parse("license"), Some(AuditFamily::License));
        assert_eq!(AuditFamily::parse("both"), None);
    }
}
