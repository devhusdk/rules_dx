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

/// Resolves a workspace-relative path and fails if it leaves the workspace.
pub fn resolve_in_workspace(
    workspace: &std::path::Path,
    rel: &str,
) -> std::io::Result<std::path::PathBuf> {
    let root = std::fs::canonicalize(workspace)?;
    let target = std::fs::canonicalize(workspace.join(rel))?;
    if target.starts_with(&root) {
        Ok(target)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path leaves the workspace",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_in_workspace_reads_a_file_inside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("apps")).unwrap();
        std::fs::write(dir.path().join("apps/uv.lock"), "x").unwrap();
        let resolved = resolve_in_workspace(dir.path(), "apps/uv.lock").unwrap();
        assert_eq!(std::fs::read_to_string(resolved).unwrap(), "x");
    }

    #[test]
    fn resolve_in_workspace_passes_not_found_through() {
        let dir = tempfile::tempdir().unwrap();
        let error = resolve_in_workspace(dir.path(), "missing.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn resolve_in_workspace_rejects_a_symlink_leaving_the_workspace() {
        let outer = tempfile::tempdir().unwrap();
        let workspace = outer.path().join("ws");
        std::fs::create_dir_all(&workspace).unwrap();
        let outside = outer.path().join("uv.lock");
        std::fs::write(&outside, "x").unwrap();
        std::os::unix::fs::symlink(&outside, workspace.join("uv.lock")).unwrap();
        let error = resolve_in_workspace(&workspace, "uv.lock").unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("leaves the workspace"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_in_workspace_follows_a_symlink_inside_the_workspace() {
        let outer = tempfile::tempdir().unwrap();
        let real = outer.path().join("real");
        std::fs::create_dir_all(real.join("locks")).unwrap();
        std::fs::write(real.join("locks/uv.lock"), "x").unwrap();
        std::os::unix::fs::symlink(real.join("locks/uv.lock"), real.join("uv.lock")).unwrap();
        let linked = outer.path().join("linked");
        std::os::unix::fs::symlink(&real, &linked).unwrap();
        let resolved = resolve_in_workspace(&linked, "uv.lock").unwrap();
        assert_eq!(std::fs::read_to_string(resolved).unwrap(), "x");
    }

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
