pub const GITLEAKS_TOOL: &str = "gitleaks";

pub const TRUFFLEHOG_V1: &str = "wont-fix";

pub const TOOL_ENV_VAR: &str = "DX_GITLEAKS_BIN";

pub const TOOL_LABEL: &str = "@dx_tools//:gitleaks";

pub fn hermetic_env(temp_dir: &std::path::Path) -> Vec<(String, String)> {
    vec![("TMPDIR".to_owned(), temp_dir.to_string_lossy().into_owned())]
}

pub const SARIF_FORMAT: &str = "sarif";

pub const REDACT_FLAG: &str = "--redact";

pub const REPORT_FORMAT_FLAG: &str = "--report-format";

pub const REPORT_PATH_FLAG: &str = "--report-path";

pub const EXIT_CODE_FLAG: &str = "--exit-code";

pub const CONFIG_FLAG: &str = "--config";

pub const CONFIG_DISCOVERY_ORDER: &[&str] = &[
    "--config flag",
    "GITLEAKS_CONFIG",
    "GITLEAKS_CONFIG_TOML",
    ".gitleaks.toml",
    "built-in defaults",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretsReport {
    pub report_path: String,
    pub config: Option<String>,
    pub exit_code: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReportProblem {
    #[error("secrets report needs an explicit --report-path destination")]
    MissingPath,
}

impl SecretsReport {
    pub fn argv(&self) -> Result<Vec<String>, ReportProblem> {
        if self.report_path.trim().is_empty() {
            return Err(ReportProblem::MissingPath);
        }
        let mut argv = vec![
            REPORT_FORMAT_FLAG.to_owned(),
            SARIF_FORMAT.to_owned(),
            REPORT_PATH_FLAG.to_owned(),
            self.report_path.clone(),
            REDACT_FLAG.to_owned(),
        ];
        if let Some(config) = &self.config {
            argv.push(CONFIG_FLAG.to_owned());
            argv.push(config.clone());
        }
        if let Some(code) = self.exit_code {
            argv.push(EXIT_CODE_FLAG.to_owned());
            argv.push(code.to_string());
        }
        Ok(argv)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretsOutcome {
    Clean,
    NeedsFindingErrorTriage,
    Failed,
}

pub fn classify_exit(code: i32) -> SecretsOutcome {
    match code {
        0 => SecretsOutcome::Clean,
        1 => SecretsOutcome::NeedsFindingErrorTriage,
        _ => SecretsOutcome::Failed,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretFinding {
    pub rule: String,
    pub message: String,
    pub path: Option<String>,
}

pub fn triage_sarif(text: &str) -> Result<Vec<SecretFinding>, String> {
    let document: serde_sarif::sarif::Sarif =
        serde_json::from_str(text).map_err(|error| format!("invalid gitleaks SARIF: {error}"))?;
    let mut findings = Vec::new();
    for run in &document.runs {
        let results = run.results.as_ref().cloned().unwrap_or_default();
        for result in &results {
            let rule = result
                .rule_id
                .as_deref()
                .unwrap_or("gitleaks/secret")
                .to_owned();
            let path = result
                .locations
                .as_ref()
                .and_then(|locations| locations.first())
                .and_then(|location| location.physical_location.as_ref())
                .and_then(|physical| physical.artifact_location.as_ref())
                .and_then(|artifact| artifact.uri.clone());
            let message = match &path {
                Some(uri) => format!("{rule} detected in {uri}"),
                None => format!("{rule} detected"),
            };
            findings.push(SecretFinding {
                rule,
                message,
                path,
            });
        }
    }
    findings.sort_by(|a, b| (&a.rule, &a.message, &a.path).cmp(&(&b.rule, &b.message, &b.path)));
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_argv_pins_sarif_redact_and_path() {
        let report = SecretsReport {
            report_path: "bazel-out/gitleaks.sarif".to_owned(),
            config: None,
            exit_code: None,
        };
        let argv = report.argv().expect("plans");
        assert_eq!(
            argv,
            vec![
                "--report-format".to_owned(),
                "sarif".to_owned(),
                "--report-path".to_owned(),
                "bazel-out/gitleaks.sarif".to_owned(),
                "--redact".to_owned(),
            ]
        );
    }

    #[test]
    fn report_argv_carries_config_and_exit_code_overrides() {
        let report = SecretsReport {
            report_path: "out.sarif".to_owned(),
            config: Some(".gitleaks.toml".to_owned()),
            exit_code: Some(2),
        };
        let argv = report.argv().expect("plans");
        assert!(argv.contains(&"--config".to_owned()));
        assert!(argv.contains(&".gitleaks.toml".to_owned()));
        assert!(argv.contains(&"--exit-code".to_owned()));
        assert!(argv.contains(&"2".to_owned()));
        assert!(argv.contains(&"sarif".to_owned()));
        assert!(argv.contains(&"--redact".to_owned()));
    }

    #[test]
    fn missing_report_path_fails_closed() {
        let report = SecretsReport {
            report_path: "  ".to_owned(),
            config: None,
            exit_code: None,
        };
        assert_eq!(report.argv(), Err(ReportProblem::MissingPath));
    }

    #[test]
    fn exit_classification_is_fail_closed_on_one() {
        assert_eq!(classify_exit(0), SecretsOutcome::Clean);
        assert_eq!(classify_exit(1), SecretsOutcome::NeedsFindingErrorTriage);
        assert_eq!(classify_exit(2), SecretsOutcome::Failed);
        assert_eq!(classify_exit(-1), SecretsOutcome::Failed);
    }

    #[test]
    fn sarif_triage_counts_results_and_never_surfaces_secrets() {
        let clean = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": []}]}"#;
        assert!(triage_sarif(clean).expect("clean").is_empty());
        let leaks = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/generic-api-key", "message": {"text": "Generic API Key"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "src/app.py"}}}]}, {"ruleId": "gitleaks/aws-key", "message": {"text": "AWS key"}, "fingerprint": "secret:AKIAIOSFODNN7EXAMPLE"}]}]}"#;
        let findings = triage_sarif(leaks).expect("leaks");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule, "gitleaks/aws-key");
        assert_eq!(findings[1].path, Some("src/app.py".to_owned()));
        for finding in &findings {
            assert!(!finding.rule.contains("AKIAIOSFODNN7EXAMPLE"));
            assert!(!finding.message.contains("AKIAIOSFODNN7EXAMPLE"));
            assert!(finding.path.as_deref() != Some("AKIAIOSFODNN7EXAMPLE"));
        }
        assert_eq!(
            findings[1].message,
            "gitleaks/generic-api-key detected in src/app.py"
        );
        assert!(triage_sarif("not json").is_err());
        assert!(triage_sarif(r#"{"version": "2.1.0"}"#).is_err());
    }

    #[test]
    fn sarif_triage_redaction_ignores_every_secret_field() {
        let aws = "AKIAIOSFODNN7EXAMPLE";
        let github = format!("{}{}", "ghp_", "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8");
        let generic = format!("{}{}", "sk-live-", "51H7x9yQ2wE4rT6yU8iO0p");
        let github: &str = &github;
        let generic: &str = &generic;
        let text = format!(
            r#"{{"version": "2.1.0", "runs": [{{"tool": {{"driver": {{"name": "gitleaks"}}}}, "results": [{{
                "ruleId": "gitleaks/aws-key",
                "message": {{"text": "leaked {aws} in src/creds.py"}},
                "fingerprints": {{"secret": "{aws}"}},
                "partialFingerprints": {{"secret/v1": "{github}"}},
                "properties": {{"secret": "{generic}"}},
                "locations": [{{
                    "physicalLocation": {{
                        "artifactLocation": {{"uri": "src/creds.py"}},
                        "region": {{"snippet": {{"text": "key = '{aws}'"}}}},
                        "contextRegion": {{"snippet": {{"text": "token {generic} here"}}}}
                    }}
                }}]}}]}}]}}"#
        );
        let findings = triage_sarif(&text).expect("unredacted triages");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.rule, "gitleaks/aws-key");
        assert_eq!(finding.path, Some("src/creds.py".to_owned()));
        assert_eq!(finding.message, "gitleaks/aws-key detected in src/creds.py");
        for secret in [aws, github, generic] {
            assert!(!finding.rule.contains(secret), "rule leaks {secret}");
            assert!(!finding.message.contains(secret), "message leaks {secret}");
            assert!(
                finding.path.as_deref().unwrap_or("").find(secret).is_none(),
                "path leaks {secret}"
            );
        }
        let redacted = text
            .replace(aws, "...")
            .replace(github, "...")
            .replace(generic, "...");
        let redacted_findings = triage_sarif(&redacted).expect("redacted triages");
        assert_eq!(findings, redacted_findings);
    }

    #[test]
    fn sarif_triage_message_never_copies_raw_text() {
        let secret_owned = format!("{}{}", "xoxb-", "123456789012-abcdefghijklmnopqrstuvwx");
        let secret: &str = &secret_owned;
        let text = format!(
            r#"{{"version": "2.1.0", "runs": [{{"tool": {{"driver": {{"name": "gitleaks"}}}}, "results": [{{
                "ruleId": "gitleaks/slack-token",
                "message": {{"text": "{secret}"}},
                "locations": [{{"physicalLocation": {{"artifactLocation": {{"uri": "src/chat.py"}}}}}}]}}]}}]}}"#
        );
        let findings = triage_sarif(&text).expect("secret-text triages");
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].message,
            "gitleaks/slack-token detected in src/chat.py"
        );
        assert!(!findings[0].message.contains(secret));
    }

    #[test]
    fn sarif_triage_handles_empty_and_multi_run_fixtures() {
        let empty_runs = r#"{"version": "2.1.0", "$schema": "https://json.schemastore.org/sarif-2.1.0.json", "runs": []}"#;
        let typed: serde_sarif::sarif::Sarif =
            serde_json::from_str(empty_runs).expect("schema-valid empty runs");
        assert!(typed.runs.is_empty());
        assert!(triage_sarif(empty_runs).expect("empty runs").is_empty());

        let multi = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/a", "message": {"text": "A"}}]}, {"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/b", "message": {"text": "B"}, "locations": [{"physicalLocation": {"artifactLocation": {"uri": "src/b.py"}}}]}]}]}"#;
        let typed_multi: serde_sarif::sarif::Sarif =
            serde_json::from_str(multi).expect("schema-valid multi-run");
        assert_eq!(typed_multi.runs.len(), 2);
        let findings = triage_sarif(multi).expect("multi-run");
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule, "gitleaks/a");
        assert_eq!(findings[1].path, Some("src/b.py".to_owned()));
    }

    #[test]
    fn frozen_spellings_and_discovery_order() {
        assert_eq!(GITLEAKS_TOOL, "gitleaks");
        assert_eq!(TRUFFLEHOG_V1, "wont-fix");
        assert_eq!(SARIF_FORMAT, "sarif");
        assert_eq!(REDACT_FLAG, "--redact");
        assert_eq!(
            CONFIG_DISCOVERY_ORDER,
            &[
                "--config flag",
                "GITLEAKS_CONFIG",
                "GITLEAKS_CONFIG_TOML",
                ".gitleaks.toml",
                "built-in defaults",
            ]
        );
    }

    #[test]
    fn hermetic_env_carries_only_tmpdir() {
        let env = hermetic_env(std::path::Path::new("/tmp/dx-audit"));
        assert_eq!(env, vec![("TMPDIR".to_owned(), "/tmp/dx-audit".to_owned())]);
        assert!(!env.iter().any(|(key, _)| key == "PATH"));
        assert!(!env.iter().any(|(key, _)| key == "GITLEAKS_CONFIG"));
        assert!(!env.iter().any(|(key, _)| key == "GITLEAKS_CONFIG_TOML"));
    }
}
