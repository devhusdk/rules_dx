use crate::secrets::{
    CONFIG_FLAG, EXIT_CODE_FLAG, REDACT_FLAG, REPORT_FORMAT_FLAG, REPORT_PATH_FLAG, SARIF_FORMAT,
};

pub const SECRETS_BINARY: &str = "gitleaks";

pub const SECRETS_SUBCOMMAND: &str = "detect";

pub const NO_GIT_FLAG: &str = "--no-git";

pub const SOURCE_FLAG: &str = "--source";

pub const WORKSPACE_SOURCE: &str = ".";

pub const SECRETS_ERROR_EXIT: &str = "2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackendPlan {
    Run {
        argv: Vec<String>,
        env: Vec<(String, String)>,
    },
    Noop,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BackendError {
    #[error("audit backend needs an explicit report destination")]
    MissingReportPath,
    #[error("audit backend needs an explicit hermetic gitleaks binary path")]
    MissingTool,
    #[error("audit backend needs an absolute gitleaks binary path, got {tool:?}")]
    NonAbsoluteTool { tool: String },
    #[error("audit backend needs an explicit temp directory for the hermetic environment")]
    MissingTempDir,
}

pub fn plan_secrets(
    tool: &str,
    report_path: &str,
    config: Option<&str>,
    temp_dir: &str,
    offline: bool,
) -> Result<BackendPlan, BackendError> {
    let _ = offline;
    if tool.trim().is_empty() {
        return Err(BackendError::MissingTool);
    }
    if !std::path::Path::new(tool).is_absolute() {
        return Err(BackendError::NonAbsoluteTool {
            tool: tool.to_owned(),
        });
    }
    if report_path.trim().is_empty() {
        return Err(BackendError::MissingReportPath);
    }
    if temp_dir.trim().is_empty() {
        return Err(BackendError::MissingTempDir);
    }
    let mut argv = vec![
        tool.to_owned(),
        SECRETS_SUBCOMMAND.to_owned(),
        NO_GIT_FLAG.to_owned(),
        SOURCE_FLAG.to_owned(),
        WORKSPACE_SOURCE.to_owned(),
        REPORT_FORMAT_FLAG.to_owned(),
        SARIF_FORMAT.to_owned(),
        REPORT_PATH_FLAG.to_owned(),
        report_path.to_owned(),
        REDACT_FLAG.to_owned(),
        EXIT_CODE_FLAG.to_owned(),
        SECRETS_ERROR_EXIT.to_owned(),
    ];
    if let Some(config) = config {
        if !config.trim().is_empty() {
            argv.push(CONFIG_FLAG.to_owned());
            argv.push(config.to_owned());
        }
    }
    Ok(BackendPlan::Run {
        argv,
        env: vec![("TMPDIR".to_owned(), temp_dir.to_owned())],
    })
}

pub fn vuln_locks(set: &str) -> &'static [&'static str] {
    match set {
        "cargo" => &["rust/tests/fixtures/hello/Cargo.lock"],
        "npm" => &["pnpm-lock.yaml", "package-lock.json", "yarn.lock"],
        "maven" => &["third_party/jvm/maven_install.json"],
        "nuget" => &["third_party/dotnet/paket.lock"],
        "go" => &["third_party/go/go.mod"],
        _ => &[],
    }
}

pub fn is_empty_set(set: &str) -> bool {
    vuln_locks(set).is_empty()
}

pub fn sets_without_coverage<'a>(sets: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = Vec::new();
    for set in sets {
        if is_empty_set(set) && !out.contains(&set) {
            out.push(set);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_plan_pins_gitleaks_sarif_redact_and_exit_split() {
        let plan = plan_secrets(
            "/hermetic/gitleaks",
            "out/gitleaks.sarif",
            None,
            "/tmp/dx",
            false,
        )
        .expect("plans");
        match plan {
            BackendPlan::Run { argv, env } => {
                assert_eq!(argv[0], "/hermetic/gitleaks");
                assert!(argv.contains(&"detect".to_owned()));
                assert!(argv.contains(&"--no-git".to_owned()));
                assert!(argv.contains(&"--source".to_owned()));
                assert!(argv.contains(&".".to_owned()));
                assert!(argv.contains(&"--report-format".to_owned()));
                assert!(argv.contains(&"sarif".to_owned()));
                assert!(argv.contains(&"--report-path".to_owned()));
                assert!(argv.contains(&"out/gitleaks.sarif".to_owned()));
                assert!(argv.contains(&"--redact".to_owned()));
                assert!(argv.contains(&"--exit-code".to_owned()));
                assert!(argv.contains(&"2".to_owned()));
                assert!(!argv.contains(&"--config".to_owned()));
                assert_eq!(env, vec![("TMPDIR".to_owned(), "/tmp/dx".to_owned())]);
            }
            BackendPlan::Noop => panic!("secrets runs gitleaks"),
        }
    }

    #[test]
    fn secrets_plan_carries_explicit_config() {
        let plan = plan_secrets(
            "/hermetic/gitleaks",
            "out.sarif",
            Some(".gitleaks.toml"),
            "/tmp/dx",
            false,
        )
        .expect("plans");
        match plan {
            BackendPlan::Run { argv, .. } => {
                assert!(argv.contains(&"--config".to_owned()));
                assert!(argv.contains(&".gitleaks.toml".to_owned()));
            }
            BackendPlan::Noop => panic!("secrets runs"),
        }
    }

    #[test]
    fn secrets_plan_rejects_empty_report_path() {
        assert_eq!(
            plan_secrets("/hermetic/gitleaks", "  ", None, "/tmp/dx", false),
            Err(BackendError::MissingReportPath)
        );
    }

    #[test]
    fn secrets_plan_rejects_bare_and_relative_tools() {
        assert_eq!(
            plan_secrets("", "out.sarif", None, "/tmp/dx", false),
            Err(BackendError::MissingTool)
        );
        assert_eq!(
            plan_secrets("gitleaks", "out.sarif", None, "/tmp/dx", false),
            Err(BackendError::NonAbsoluteTool {
                tool: "gitleaks".to_owned()
            })
        );
        assert_eq!(
            plan_secrets("tools/gitleaks", "out.sarif", None, "/tmp/dx", false),
            Err(BackendError::NonAbsoluteTool {
                tool: "tools/gitleaks".to_owned()
            })
        );
        assert_eq!(
            plan_secrets("/hermetic/gitleaks", "out.sarif", None, "  ", false),
            Err(BackendError::MissingTempDir)
        );
    }

    #[test]
    fn secrets_plan_env_is_sanitized_tmpdir_only() {
        let plan = plan_secrets(
            "/hermetic/gitleaks",
            "out.sarif",
            None,
            "/tmp/dx-run",
            false,
        )
        .expect("plans");
        match plan {
            BackendPlan::Run { env, .. } => {
                assert_eq!(env, vec![("TMPDIR".to_owned(), "/tmp/dx-run".to_owned())]);
                assert!(!env.iter().any(|(key, _)| key == "PATH"));
            }
            BackendPlan::Noop => panic!("secrets runs"),
        }
    }

    #[test]
    fn vuln_locks_pin_per_set_coverage() {
        assert_eq!(
            vuln_locks("cargo"),
            &["rust/tests/fixtures/hello/Cargo.lock"]
        );
        assert_eq!(
            vuln_locks("npm"),
            &["pnpm-lock.yaml", "package-lock.json", "yarn.lock"]
        );
        assert_eq!(vuln_locks("maven"), &["third_party/jvm/maven_install.json"]);
        assert_eq!(vuln_locks("nuget"), &["third_party/dotnet/paket.lock"]);
        assert_eq!(vuln_locks("go"), &["third_party/go/go.mod"]);
        assert!(!is_empty_set("go"));
        assert!(!is_empty_set("cargo"));
        assert!(is_empty_set("unknown-set"));
    }

    #[test]
    fn sets_without_coverage_names_each_gap_once() {
        assert_eq!(
            sets_without_coverage(["npm-adopt", "cargo", "npm-adopt", "uv", "npm"]),
            vec!["npm-adopt", "uv"]
        );
        assert!(sets_without_coverage(["cargo", "npm", "maven", "nuget", "go"]).is_empty());
        assert_eq!(sets_without_coverage(["unknown-set"]), vec!["unknown-set"]);
    }

    #[test]
    fn frozen_spellings() {
        assert_eq!(SECRETS_BINARY, "gitleaks");
        assert_eq!(SECRETS_SUBCOMMAND, "detect");
        assert_eq!(NO_GIT_FLAG, "--no-git");
        assert_eq!(SOURCE_FLAG, "--source");
        assert_eq!(WORKSPACE_SOURCE, ".");
        assert_eq!(SECRETS_ERROR_EXIT, "2");
    }

    #[test]
    fn secrets_plan_is_cache_only_identical_offline() {
        let online = plan_secrets("/hermetic/gitleaks", "out.sarif", None, "/tmp/dx", false)
            .expect("online plans");
        let offline = plan_secrets("/hermetic/gitleaks", "out.sarif", None, "/tmp/dx", true)
            .expect("offline plans");
        assert_eq!(online, offline);
        let online_configed = plan_secrets(
            "/hermetic/gitleaks",
            "out.sarif",
            Some(".gitleaks.toml"),
            "/tmp/dx",
            false,
        )
        .expect("online configed");
        let offline_configed = plan_secrets(
            "/hermetic/gitleaks",
            "out.sarif",
            Some(".gitleaks.toml"),
            "/tmp/dx",
            true,
        )
        .expect("offline configed");
        assert_eq!(online_configed, offline_configed);
    }
}
