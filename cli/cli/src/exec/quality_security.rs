use dx_output::{DiagnosticEvent, Severity};
use std::path::Path;

const TOOL: &str = "trivy_config";

/// Drops trivy_config findings covered by an unexpired `[[exception]]` with
/// `set = "trivy_config"`, `advisory` naming the trivy ID, `package` naming the
/// file or `*`, and `versions = "*"`.
pub(super) fn drop_excepted_findings(
    workspace: &Path,
    audit_today: Option<&str>,
    status: &mut Vec<DiagnosticEvent>,
) {
    if !status.iter().any(|diagnostic| diagnostic.tool == TOOL) {
        return;
    }
    let policy = match super::audit::load_security_policy(workspace) {
        Ok(policy) => policy,
        Err(error) => {
            status.push(DiagnosticEvent {
                path: Some("security.toml".to_owned()),
                ..DiagnosticEvent::new(Severity::Error, TOOL, error.to_string())
            });
            return;
        }
    };
    let today = super::audit::today_utc(audit_today);
    let live = dx_audit::exception::live_exceptions_for_set(&policy, TOOL, &today);
    if live.is_empty() {
        return;
    }
    status.retain(|diagnostic| {
        if diagnostic.tool != TOOL {
            return true;
        }
        let path = diagnostic.path.as_deref().unwrap_or("").replace('\\', "/");
        !live.iter().any(|exception| {
            exception.versions.trim() == "*"
                && diagnostic.rule.as_deref() == Some(exception.advisory.as_str())
                && (exception.package == "*" || exception.package == path)
        })
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: Option<&str> = Some("2026-11-06");

    fn finding(tool: &str, rule: &str, path: &str) -> DiagnosticEvent {
        DiagnosticEvent {
            rule: Some(rule.to_owned()),
            path: Some(path.to_owned()),
            ..DiagnosticEvent::new(Severity::Error, tool, "m")
        }
    }

    fn exception(
        advisory: &str,
        package: &str,
        set: &str,
        versions: &str,
        expires: &str,
    ) -> String {
        format!(
            "schema_version = 1\n[[exception]]\nadvisory = \"{advisory}\"\npackage = \"{package}\"\nset = \"{set}\"\nversions = \"{versions}\"\nreason = \"accepted\"\nexpires = \"{expires}\"\n"
        )
    }

    fn run(security_toml: Option<&str>, findings: Vec<DiagnosticEvent>) -> Vec<DiagnosticEvent> {
        let workspace = tempfile::tempdir().expect("tempdir");
        if let Some(text) = security_toml {
            std::fs::write(workspace.path().join("security.toml"), text).expect("write");
        }
        let mut status = findings;
        drop_excepted_findings(workspace.path(), TODAY, &mut status);
        status
    }

    fn sample() -> Vec<DiagnosticEvent> {
        vec![finding(TOOL, "DS-0002", "app/Dockerfile")]
    }

    #[test]
    fn matching_exception_drops_the_finding() {
        let text = exception("DS-0002", "app/Dockerfile", TOOL, "*", "2027-01-01");
        assert!(run(Some(&text), sample()).is_empty());
    }

    #[test]
    fn star_package_covers_every_path() {
        let text = exception("DS-0002", "*", TOOL, "*", "2027-01-01");
        let findings = vec![
            finding(TOOL, "DS-0002", "app/Dockerfile"),
            finding(TOOL, "DS-0002", "web/Dockerfile"),
        ];
        assert!(run(Some(&text), findings).is_empty());
    }

    #[test]
    fn findings_stay_when_the_exception_does_not_fit() {
        for text in [
            exception("DS-0002", "app/Dockerfile", TOOL, "*", "2026-11-06"),
            exception("DS-0002", "app/Dockerfile", "oci", "*", "2027-01-01"),
            exception("DS-0002", "app/Dockerfile", TOOL, "1.0", "2027-01-01"),
            exception("DS-0002", "web/Dockerfile", TOOL, "*", "2027-01-01"),
        ] {
            assert_eq!(run(Some(&text), sample()), sample(), "{text}");
        }
    }

    #[test]
    fn other_tools_with_the_same_id_are_kept() {
        let text = exception("DS-0002", "*", TOOL, "*", "2027-01-01");
        let findings = vec![finding("hadolint", "DS-0002", "app/Dockerfile")];
        assert_eq!(run(Some(&text), findings.clone()), findings);
    }

    #[test]
    fn missing_security_toml_keeps_findings() {
        assert_eq!(run(None, sample()), sample());
    }

    #[test]
    fn invalid_security_toml_adds_an_error_on_it() {
        let status = run(Some("[[exception]\n"), sample());
        assert_eq!(status.len(), 2);
        assert_eq!(status[1].path.as_deref(), Some("security.toml"));
        assert_eq!(status[1].severity, Severity::Error);
    }
}
