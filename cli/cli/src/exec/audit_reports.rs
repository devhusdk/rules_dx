use super::super::test_support::*;
use super::audit_live::*;
use crate::exec::execute;

#[test]
fn secrets_launch_failure_is_incomplete_without_a_clean_result() {
    struct Unavailable;
    impl dx_process::Runner for Unavailable {
        fn pinned_tool(&self, var: &str) -> dx_process::PinnedVar {
            if var == dx_audit::secrets::TOOL_ENV_VAR {
                dx_process::PinnedVar::Path(std::path::PathBuf::from(hermetic_gitleaks()))
            } else {
                dx_process::PinnedVar::Unset
            }
        }
        fn run(
            &self,
            _: &[String],
            _: &std::path::Path,
            _: &[(&str, &str)],
        ) -> std::io::Result<dx_process::ChildStatus> {
            Err(std::io::Error::other("cannot spawn pinned tool"))
        }
        fn run_hermetic(
            &self,
            argv: &[String],
            cwd: &std::path::Path,
            env: &[(&str, &str)],
        ) -> std::io::Result<dx_process::ChildStatus> {
            self.run(argv, cwd, env)
        }
    }
    let harness = Harness::new("audit-launch-failed");
    let bazel = super::QueryBazel {
        query_runner: &harness.query,
        workspace: &harness.workspace,
        startup: &[],
    };
    let (findings, incomplete, diagnostics) = super::run_secrets(
        &harness.workspace,
        &Unavailable,
        &harness.query,
        &bazel,
        &harness.temp,
        1,
        0,
        false,
    );
    assert!(findings.is_empty());
    assert!(diagnostics.is_empty());
    assert!(
        incomplete
            .expect("incomplete")
            .contains("failed to launch secrets auditor")
    );
}

#[test]
fn license_policy_read_failures_and_empty_default_are_explicit() {
    let harness = Harness::new("license-policy-io");
    let path = harness.workspace.join("license.toml");
    std::fs::write(&path, " \n").expect("empty");
    assert!(
        super::load_license_policy(&harness.workspace)
            .expect("default")
            .tables
            .allow
            .contains("MIT")
    );
    std::fs::write(&path, [0xff]).expect("invalid utf8");
    assert!(matches!(
        super::load_license_policy(&harness.workspace),
        Err(super::AuditError::NotUtf8 { .. })
    ));
    std::fs::remove_file(&path).expect("remove");
    std::fs::create_dir(&path).expect("directory");
    assert!(matches!(
        super::load_license_policy(&harness.workspace),
        Err(super::AuditError::Read { .. })
    ));
}

fn write_demo_license(harness: &Harness, license: &str) {
    harness.write_source(
        "cc/context/Cargo.lock",
        "[[package]]\nname = \"demo\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
    );
    harness.write_source(
        "cargo-bazel-lock.json",
        &serde_json::json!({"packages": {"demo 1.0.0": {"license": license}}}).to_string(),
    );
}

#[test]
fn the_built_in_license_table_passes_only_its_allow_list() {
    for (license, rule) in [
        ("ISC", ""),
        ("Unlicense", ""),
        ("MIT", "license/missing-notice-text"),
        ("Apache-2.0", "license/missing-notice-text"),
        ("BSD-2-Clause", "license/missing-notice-text"),
        ("BSD-3-Clause", "license/missing-notice-text"),
        ("AGPL-3.0-only", "license/AGPL-3.0-only"),
        ("AGPL-3.0-or-later", "license/AGPL-3.0-or-later"),
        ("SSPL-1.0", "license/SSPL-1.0"),
        ("LGPL-2.1-only", "license/LGPL-2.1-only"),
        ("LGPL-3.0-or-later", "license/LGPL-3.0-or-later"),
        ("MPL-2.0", "license/MPL-2.0"),
        ("EPL-2.0", "license/EPL-2.0"),
        ("CDDL-1.0", "license/CDDL-1.0"),
        ("GPL-2.0-only", "license/GPL-2.0-only"),
        ("GPL-3.0-or-later", "license/GPL-3.0-or-later"),
        ("Zlib", "license/Zlib"),
    ] {
        let (code, out, err) = run_with(
            &[
                "license",
                "//rust/tests/fixtures/hello:demo",
                "--report=sarif=-",
            ],
            &AuditRunner::clean(),
            &|harness| {
                write_demo_license(harness, license);
            },
        );
        if rule.is_empty() {
            assert_eq!(
                code, 0,
                "{license} is on the built-in allow list: {out}{err}"
            );
            continue;
        }
        assert_eq!(code, 1, "{license} is not on it: {out}{err}");
        assert!(
            out.contains(rule),
            "{license} must report {rule}: {out}{err}"
        );
    }
}

#[test]
fn the_strict_profile_denies_a_license_the_standard_profile_only_reviews() {
    for (profile, reviewed) in [("standard", true), ("strict", false)] {
        let harness = Harness::new("license-profile");
        harness.write_source(
            "dx.toml",
            &format!("schema_version = 1\ntier = \"{profile}\"\n"),
        );
        let tables = super::load_license_policy(&harness.workspace)
            .expect("policy")
            .tables;
        assert_eq!(tables.review.contains("MPL-2.0"), reviewed, "{profile}");
        assert_eq!(tables.deny.contains("MPL-2.0"), !reviewed, "{profile}");
    }
}

#[test]
fn an_invalid_dx_toml_fails_both_policies() {
    let harness = Harness::new("audit-bad-dx-toml");
    harness.write_source("dx.toml", "tier = \"strict\"\n");
    let security = super::load_security_policy(&harness.workspace).expect_err("security");
    assert!(matches!(security, super::AuditError::SecurityInvalid(_)));
    assert!(
        security.to_string().contains("dx.toml: ")
            && security.to_string().contains("schema_version is missing"),
        "{security}"
    );
    let license = super::load_license_policy(&harness.workspace).expect_err("license");
    assert!(matches!(license, super::AuditError::LicenseInvalid { .. }));
    assert!(license.to_string().contains("dx.toml: "), "{license}");
}

#[test]
fn an_internal_license_scope_skips_the_review_and_notice_tables() {
    let policy = "schema_version = 1\n[policy]\nblocked = [\"AGPL-3.0-only\"]\n[policy.distributed]\nallow = []\nreview = []\ndeny = []\n[distribution]\ninternal = [\"//rust/tests/fixtures/hello:internal\"]\n";
    for (license, code) in [("Zlib", 0), ("ISC", 0), ("MIT", 0), ("AGPL-3.0-only", 1)] {
        let (code_seen, out, err) = run_with(
            &[
                "license",
                "//rust/tests/fixtures/hello:internal",
                "--report=sarif=-",
            ],
            &AuditRunner::clean(),
            &|harness| {
                write_demo_license(harness, license);
                harness.write_source("license.toml", policy);
            },
        );
        assert_eq!(code_seen, code, "{license} when internal: {out}{err}");
    }
    for (license, rule) in [
        ("Zlib", "license/Zlib"),
        ("ISC", "license/ISC"),
        ("MIT", "license/missing-notice-text"),
        ("AGPL-3.0-only", "license/AGPL-3.0-only"),
    ] {
        let (code, out, err) = run_with(
            &[
                "license",
                "//rust/tests/fixtures/hello:distributed",
                "--report=sarif=-",
            ],
            &AuditRunner::clean(),
            &|harness| {
                write_demo_license(harness, license);
                harness.write_source("license.toml", policy);
            },
        );
        assert_eq!(code, 1, "{license} when distributed: {out}{err}");
        assert!(
            out.contains(rule),
            "{license} must report {rule}: {out}{err}"
        );
    }
}

#[test]
fn a_repo_wide_internal_policy_covers_sub_scopes_and_affected_labels() {
    let policy = "schema_version = 1\n[policy]\nblocked = [\"AGPL-3.0-only\"]\n[policy.distributed]\nallow = []\nreview = []\ndeny = []\n[distribution]\ninternal = [\"//...\"]\n";
    for scope in [
        "//rust/tests/fixtures/hello:all",
        "//rust/tests/fixtures/hello:app",
    ] {
        for (license, code) in [("MIT", 0), ("AGPL-3.0-only", 1)] {
            let (code_seen, out, err) = run_with(
                &["license", scope, "--report=sarif=-"],
                &AuditRunner::clean(),
                &|harness| {
                    write_demo_license(harness, license);
                    harness.write_source("license.toml", policy);
                },
            );
            assert_eq!(code_seen, code, "{license} under {scope}: {out}{err}");
            assert!(
                !out.contains("license/missing-notice-text"),
                "{license} under {scope}: {out}{err}"
            );
        }
    }
    let load = |text: &str| {
        dx_audit::license_policy::load_licenses(
            Some(text),
            dx_audit::profile_base::Profile::Standard,
        )
        .expect("policy")
    };
    let roots = |labels: &[&str]| {
        labels
            .iter()
            .map(|label| (*label).to_owned())
            .collect::<Vec<_>>()
    };
    let wide = load(policy);
    assert_eq!(
        super::tier_for_roots(&wide, &roots(&["//cli/cli:dx_cli", "//cli/audit:dx_audit"])),
        dx_audit::license_expr::Tier::Internal
    );
    let narrow = load("schema_version = 1\n[distribution]\ninternal = [\"//cli/...\"]\n");
    assert_eq!(
        super::tier_for_roots(&narrow, &roots(&["//cli/cli:dx_cli", "//go:x"])),
        dx_audit::license_expr::Tier::Distributed
    );
}

#[test]
fn security_policy_read_failures_and_absent_file_are_explicit() {
    let harness = Harness::new("security-policy-io");
    assert!(
        super::load_security_policy(&harness.workspace)
            .expect("absent file")
            .exceptions
            .is_empty()
    );
    let path = harness.workspace.join("security.toml");
    std::fs::write(&path, " \n").expect("empty");
    assert!(
        super::load_security_policy(&harness.workspace)
            .expect("empty file")
            .exceptions
            .is_empty()
    );
    std::fs::write(&path, [0xff]).expect("invalid utf8");
    assert!(matches!(
        super::load_security_policy(&harness.workspace),
        Err(super::AuditError::NotUtf8 { .. })
    ));
    std::fs::remove_file(&path).expect("remove");
    std::fs::create_dir(&path).expect("directory");
    assert!(matches!(
        super::load_security_policy(&harness.workspace),
        Err(super::AuditError::Read { .. })
    ));
}

#[test]
fn license_exceptions_only_approve_matching_current_findings() {
    for (license, exception, expected) in [
        ("AGPL-3.0-only", "", 1),
        ("GPL-3.0-only", "", 1),
        ("MPL-2.0", "", 1),
        (
            "GPL-3.0-only",
            "[[exception]]\npackage = \"demo\"\nset = \"cargo\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\nreason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
            0,
        ),
        (
            "GPL-3.0-only",
            "[[exception]]\npackage = \"other\"\nset = \"cargo\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\nreason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
            1,
        ),
        (
            "GPL-3.0-only",
            "[[exception]]\npackage = \"demo\"\nset = \"cargo\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\nreason = \"reviewed\"\nexpires = \"2000-01-01\"\n",
            1,
        ),
    ] {
        let (code, out, err) = run_with(
            &[
                "license",
                "//rust/tests/fixtures/hello:demo",
                "--output=json",
            ],
            &AuditRunner::clean(),
            &|h| {
                write_demo_license(h, license);
                h.write_source("license.toml", &format!("schema_version = 1\n[policy]\nblocked = [\"AGPL-3.0-only\"]\n[policy.distributed]\nallow = []\ndeny = [\"GPL-3.0-only\"]\nreview = [\"MPL-2.0\"]\n{exception}\n[[inventory]]\npackage = \"demo\"\nset = \"cargo\"\nlicense = \"{license}\"\nversions = \"1.0.0\"\ntext_present = true\n"));
            },
        );
        assert_eq!(code, expected, "{license}: {out}{err}");
    }
    let (code, out, err) = run_with(
        &["license", "//rust/tests/fixtures/hello:demo"],
        &AuditRunner::clean(),
        &|h| {
            h.write_source("cargo-bazel-lock.json", "{}");
            h.write_source("cc/context/Cargo.lock", "{");
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        err.contains("could not parse cc/context/Cargo.lock"),
        "{err}"
    );
}

#[test]
fn license_accept_writes_an_exception_that_covers_the_finding() {
    let mut harness = Harness::new("audit-license-accept");
    harness.audit_today = Some(TEST_AUDIT_TODAY.to_owned());
    write_hubs(&harness);
    write_demo_license(&harness, "GPL-3.0-only");
    let policy = "# keep this comment\nschema_version = 1\n[policy.distributed]\nallow = []\ndeny = [\"GPL-3.0-only\"]\n\n[[inventory]]\npackage = \"demo\"\nset = \"cargo\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\ntext_present = true\n";
    harness.write_source("license.toml", policy);
    let run = |extra: &[&str]| {
        let mut argv = vec![
            "license",
            "//rust/tests/fixtures/hello:demo",
            "--output=json",
        ];
        argv.extend_from_slice(extra);
        let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        let invocation = dx_plan::args::parse(&words).expect("parse");
        harness.execute_with(&invocation, &AuditRunner::clean())
    };
    let accept = [
        "--accept",
        "demo",
        "--reason",
        "reviewed",
        "--expires",
        "2999-01-01",
    ];
    assert_eq!(run(&[]).0, 1);
    let (code, out, err) = run(&accept);
    assert_eq!(code, 0, "{out}{err}");
    let written = std::fs::read_to_string(harness.workspace.join("license.toml")).expect("file");
    assert!(written.starts_with(policy), "{written}");
    assert!(written.contains("versions = \"=1.0.0\""), "{written}");
    assert_eq!(run(&[]).0, 0);
    assert_eq!(run(&accept).0, 0);
    let again = std::fs::read_to_string(harness.workspace.join("license.toml")).expect("file");
    assert_eq!(again, written);
}

#[test]
fn secrets_process_status_and_report_content_are_both_authoritative() {
    let clean =
        r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks"}},"results":[]}]}"#;
    let finding = r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks"}},"results":[{"ruleId":"gitleaks/token","message":{"text":"token detected"}}]}]}"#;
    for (status, sarif, expected, detail) in [
        (Some(0), Some(clean), 0, ""),
        (Some(0), Some(finding), 1, "1 secret findings"),
        (Some(0), Some("{"), 1, "invalid gitleaks SARIF"),
        (Some(1), Some("{"), 1, "invalid gitleaks SARIF"),
        (Some(1), Some(clean), 1, "no SARIF results"),
        (Some(1), None, 1, "without a SARIF report"),
        (Some(7), None, 1, "exited 7"),
        (None, None, 1, "terminated by signal"),
    ] {
        let runner = AuditRunner {
            code: status,
            sarif: sarif.map(str::to_owned),
            ..AuditRunner::clean()
        };
        let (code, out, err) = run_with(&["security", "--output=json"], &runner, &|h| {
            clean_workspace_with_advisories(h);
        });
        assert_eq!(
            code, expected,
            "status={status:?} sarif={sarif:?}: {out}{err}"
        );
        assert!(err.contains(detail), "{err}");
        let events: Vec<serde_json::Value> = out
            .lines()
            .map(|line| serde_json::from_str(line).expect("event"))
            .collect();
        assert_eq!(events.last().expect("finished")["exit_code"], expected);
    }
}

#[test]
fn json_rejects_gitleaks_findings_with_unsafe_paths() {
    for uri in ["/abs/creds.py", "../x.py"] {
        let sarif = format!(
            r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"gitleaks"}}}},"results":[{{"ruleId":"gitleaks/token","message":{{"text":"token detected"}},"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{uri}"}}}}}}]}}]}}]}}"#
        );
        let runner = AuditRunner::with_sarif(Some(0), &sarif);
        let (code, out, err) = run_with(&["security", "--output=json"], &runner, &|h| {
            clean_workspace_with_advisories(h);
        });
        assert_eq!(code, 1, "{uri}: {out}{err}");
        let events = json_events(&out);
        assert!(
            events.iter().any(|event| event["code"] == "invalid_result"
                && event["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("invalid gitleaks finding"))),
            "{uri}: {out}"
        );
        assert_eq!(
            events.last().expect("finished")["exit_code"],
            serde_json::json!(1)
        );
    }
}

#[test]
fn audit_stdout_reports_are_single_machine_documents() {
    for family in ["security", "license"] {
        let report = "--report=sarif=-".to_owned();
        let (code, out, err) = run_with(&[family, &report], &AuditRunner::clean(), &|h| {
            clean_workspace_with_advisories(h);
        });
        assert_eq!(code, if family == "license" { 1 } else { 0 }, "{out}{err}");
        let value: serde_json::Value = serde_json::from_str(&out).expect("one JSON document");
        assert_eq!(value["version"], "2.1.0");
    }
}

const CLEAN_SARIF: &str =
    r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks"}},"results":[]}]}"#;

/// Runs the secrets family against a fake gitleaks and returns its argv and diagnostics.
fn scan_secrets(harness: &Harness) -> (Vec<String>, Vec<dx_output::DiagnosticEvent>) {
    let runner = AuditRunner::with_sarif(Some(0), CLEAN_SARIF);
    let bazel = super::QueryBazel {
        query_runner: &harness.query,
        workspace: &harness.workspace,
        startup: &[],
    };
    let (findings, incomplete, diagnostics) = super::run_secrets(
        &harness.workspace,
        &runner,
        &harness.query,
        &bazel,
        &harness.temp,
        7,
        3,
        false,
    );
    assert!(findings.is_empty());
    assert_eq!(incomplete, None);
    let calls = runner.calls.borrow();
    assert_eq!(calls.len(), 1, "{calls:?}");
    (calls[0].clone(), diagnostics)
}

fn config_arg(argv: &[String]) -> Option<String> {
    let index = argv.iter().position(|arg| arg == "--config")?;
    argv.get(index + 1).cloned()
}

fn ignored_query_argv() -> Vec<String> {
    std::iter::once("git")
        .chain(dx_audit::secrets::IGNORED_QUERY)
        .map(ToOwned::to_owned)
        .collect()
}

const USER_CONFIG: &str = "[extend]\npath = \".dx/config/gitleaks/base.toml\"\n\n[allowlist]\ndescription = \"fixtures\"\nregexes = [\"example\"]\n";

#[test]
fn secrets_scan_skips_git_ignored_paths() {
    let harness = Harness::new("audit-secrets-ignored");
    harness.query.script_git(0, "ign/\0local.env\0");
    harness.write_source(".gitleaks.toml", USER_CONFIG);
    let (argv, diagnostics) = scan_secrets(&harness);
    assert_eq!(
        *harness.query.git_calls.borrow(),
        vec![ignored_query_argv()]
    );
    assert!(harness.query.calls.borrow().is_empty());
    let config = config_arg(&argv).expect("--config");
    let config = std::path::PathBuf::from(config);
    assert!(config.is_absolute(), "{config:?}");
    assert!(config.starts_with(&harness.temp), "{config:?}");
    let written = std::fs::read_to_string(&config).expect("config");
    let base = std::path::absolute(harness.workspace.join(dx_audit::secrets::BASE_CONFIG_PATH))
        .expect("absolute base");
    let expected = dx_audit::secrets::scoped_config(
        Some(USER_CONFIG),
        &[r"^ign(?:/|$)".to_owned(), r"^local\.env$".to_owned()],
        &base.to_string_lossy(),
    )
    .expect("expected config");
    assert_eq!(written, expected);
    assert!(written.contains("[[allowlists]]"), "{written}");
    assert!(!written.contains("[allowlist]"), "{written}");
    assert!(written.contains("fixtures"), "{written}");
    assert!(
        diagnostics
            .iter()
            .any(|event| event.message.contains("without a hash pin")
                && event.path.as_deref() == Some(".gitleaks.toml"))
    );
}

#[test]
fn secrets_scan_without_user_config_scopes_the_default_rules() {
    let harness = Harness::new("audit-secrets-ignored-default");
    harness.query.script_git(0, "ign/\0");
    let (argv, diagnostics) = scan_secrets(&harness);
    let config = config_arg(&argv).expect("--config");
    let written = std::fs::read_to_string(config).expect("config");
    assert!(written.contains("[extend]"), "{written}");
    assert!(written.contains("base.toml"), "{written}");
    assert!(!written.contains("useDefault"), "{written}");
    assert!(diagnostics.is_empty());
}

#[test]
fn secrets_scan_always_passes_a_config_and_an_empty_ignore_path() {
    let harness = Harness::new("audit-secrets-always-config");
    harness.query.script_git(0, "");
    harness.write_source(".gitleaksignore", "anything\n");
    let (argv, _) = scan_secrets(&harness);
    assert!(config_arg(&argv).is_some(), "{argv:?}");
    let index = argv
        .iter()
        .position(|arg| arg == "--gitleaks-ignore-path")
        .expect("--gitleaks-ignore-path");
    assert!(!std::path::Path::new(&argv[index + 1]).exists());
}

#[test]
fn secrets_scan_fails_on_an_allowlist_without_a_description() {
    let harness = Harness::new("audit-secrets-no-description");
    harness.query.script_git(0, "");
    harness.write_source(
        ".gitleaks.toml",
        "[extend]\npath = \".dx/config/gitleaks/base.toml\"\n\n[[allowlists]]\npaths = [\"a\"]\n",
    );
    let runner = AuditRunner::with_sarif(Some(0), CLEAN_SARIF);
    let bazel = super::QueryBazel {
        query_runner: &harness.query,
        workspace: &harness.workspace,
        startup: &[],
    };
    let (_, incomplete, _) = super::run_secrets(
        &harness.workspace,
        &runner,
        &harness.query,
        &bazel,
        &harness.temp,
        7,
        3,
        false,
    );
    let error = incomplete.expect("config error");
    assert!(error.contains(".gitleaks.toml:4:"), "{error}");
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn secrets_scan_writes_the_base_config_when_git_gives_nothing() {
    for case in ["exit-128", "empty", "unscripted"] {
        for user_config in [true, false] {
            let harness = Harness::new(&format!("audit-secrets-fallback-{case}-{user_config}"));
            match case {
                "exit-128" => harness.query.script_git(128, ""),
                "empty" => harness.query.script_git(0, ""),
                _ => {}
            }
            if user_config {
                harness.write_source(
                    ".gitleaks.toml",
                    "[extend]\npath = \".dx/config/gitleaks/base.toml\"\n",
                );
            }
            let (argv, _) = scan_secrets(&harness);
            let config = config_arg(&argv).expect("--config");
            let written = std::fs::read_to_string(config).expect("config");
            assert!(
                written.contains("base.toml"),
                "{case} {user_config}: {written}"
            );
            assert!(!written.contains("allowlists"), "{written}");
        }
    }
}

#[test]
fn committed_gitleaks_config_produces_a_trust_warning() {
    {
        let config = ".gitleaks.toml";
        let runner = AuditRunner::with_sarif(
            Some(0),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks"}},"results":[]}]}"#,
        );
        let (code, out, err) = run_with(&["security", "--report=sarif=-"], &runner, &|h| {
            clean_workspace_with_advisories(h);
            h.write_source(
                config,
                "[extend]\npath = \".dx/config/gitleaks/base.toml\"\n",
            );
        });
        assert_eq!(code, 0, "{out}{err}");
        let value: serde_json::Value = serde_json::from_str(&out).expect("sarif");
        assert!(value.to_string().contains("without a hash pin"));
        assert!(
            runner
                .calls
                .borrow()
                .iter()
                .any(|args| args.iter().any(|arg| arg == "--config"))
        );
    }
}

#[test]
fn advisory_loader_rejects_corrupt_pinned_snapshots() {
    use super::AuditError;
    for variant in ["directory", "tampered", "invalid-json", "no-pin"] {
        let harness = Harness::new(&format!("advisory-corrupt-{variant}"));
        let json = if variant == "invalid-json" { "{" } else { "[]" };
        write_advisory(&harness, "cargo", json);
        let sha = dx_digest::sha256_hex(json.as_bytes());
        let cached = harness
            .workspace
            .join(format!(".dx/local/advisory/{sha}/cargo.json"));
        match variant {
            "directory" => {
                std::fs::remove_file(&cached).expect("remove snapshot");
                std::fs::create_dir(&cached).expect("directory snapshot");
            }
            "tampered" => std::fs::write(&cached, "[ ]").expect("tamper"),
            "no-pin" => std::fs::remove_file(harness.workspace.join(".dx/pins")).expect("pins"),
            _ => {}
        }
        let error =
            super::load_advisories(&harness.workspace, "cargo", "cargo", false).expect_err(variant);
        let expected = match variant {
            "directory" => matches!(error, AuditError::AdvisoryPinFetch { .. }),
            "tampered" => matches!(error, AuditError::AdvisoryPinMismatch { .. }),
            "invalid-json" => matches!(error, AuditError::AdvisoryParse { .. }),
            "no-pin" => matches!(error, AuditError::AdvisoryPinMissing { .. }),
            _ => unreachable!(),
        };
        assert!(expected, "{variant}: {error}");
    }
}

#[test]
fn audit_live_names_the_lockfile_each_finding_came_from() {
    use dx_plan::args::parse;
    fn sarif_paths(
        harness: &Harness,
        report: &str,
        driver: &str,
    ) -> std::collections::BTreeMap<String, String> {
        let sarif = std::fs::read_to_string(harness.workspace.join(report)).expect("sarif");
        let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
        let runs = value["runs"].as_array().expect("runs");
        let run = runs
            .iter()
            .find(|run| {
                run["tool"]["driver"]["name"] == serde_json::Value::String(driver.to_owned())
            })
            .unwrap_or_else(|| panic!("{driver} run in {value}"));
        let mut by_package: std::collections::BTreeMap<String, String> = Default::default();
        for result in run["results"].as_array().expect("results") {
            let message = result["message"]["text"].as_str().expect("message");
            let package = message.split('@').next().expect("package").to_owned();
            by_package.insert(
                package,
                result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                    .as_str()
                    .expect("uri")
                    .to_owned(),
            );
        }
        by_package
    }
    let advisories = serde_json::json!([
        {"id": "GHSA-aaaa", "package": "pnpmonly", "versions": "1.0.0", "severity": "high", "fixed": ["1.0.1"], "set": "npm"},
    ])
    .to_string();
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-lock-paths");
    write_hubs(&harness);
    harness.write_source(
        "pnpm-lock.yaml",
        "lockfileVersion: '9.0'\n\npackages:\n\n  'pnpmonly@1.0.0':\n    resolution: {integrity: sha512-abc}\n",
    );
    harness.write_source(
        "license.toml",
        "schema_version = 1\n[policy]\nblocked = [\"AGPL-3.0-only\"]\n[policy.distributed]\nallow = []\ndeny = [\"GPL-3.0-only\"]\nreview = []\n[[inventory]]\npackage = \"pnpmonly\"\nset = \"cargo\"\nlicense = \"AGPL-3.0-only\"\nversions = \"1.0.0\"\ntext_present = false\n[[inventory]]\npackage = \"yarnonly\"\nset = \"cargo\"\nlicense = \"AGPL-3.0-only\"\nversions = \"3.0.0\"\ntext_present = false\n",
    );
    write_advisory(&harness, "npm", &advisories);
    {
        let (command, report, driver) = ("security", "vuln.sarif", "vuln");
        let invocation = parse(&[
            command.to_owned(),
            "//:demo".to_owned(),
            format!("--report=sarif={report}"),
        ])
        .expect("parse");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(&invocation, harness.env(&runner, &mut out, &mut err));
        let out_text = String::from_utf8(out).expect("stdout");
        let err_text = String::from_utf8(err).expect("stderr");
        assert_eq!(code, 1, "{command}: {out_text}{err_text}");
        let by_package = sarif_paths(&harness, report, driver);
        assert_eq!(
            by_package.get("pnpmonly").map(String::as_str),
            Some("pnpm-lock.yaml"),
            "{command}: {by_package:?}"
        );
    }
}

#[test]
fn audit_live_unowned_scope_fails_usage() {
    let harness = Harness::new("audit-unowned");
    let (code, _out, err) = harness.run(&["security", "docs/cli/README.md"]);
    assert_eq!(code, 2, "{err}");
}

#[test]
fn audit_live_json_emits_per_family_lifecycle() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(&["security", "--output=json"], &runner, &|harness| {
        write_all_lock_families_with_advisories(harness);
    });
    assert_eq!(code, 0, "{out}{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert!(kinds.contains(&"notice"));
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(0)
    );
}

#[test]
fn audit_live_json_failure_emits_error_and_finished_one() {
    let sarif = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/aws-key", "message": {"text": "AWS key"}}]}]}"#;
    let runner = AuditRunner::with_sarif(Some(1), sarif);
    let (code, out, err) = run_with(&["security", "--output=json"], &runner, &|harness| {
        write_all_lock_families_with_advisories(harness);
        write_cargo_license_lock(harness);
        write_mit_licenses(harness);
    });
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("audit_failed"), "{err}");
    let events = json_events(&out);
    let kinds = event_kinds(&events);
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert!(kinds.contains(&"error"));
    let gitleaks = finding(&events, "gitleaks");
    assert!(gitleaks.get("package").is_none(), "{gitleaks}");
    assert_eq!(
        event(&events, "error")["code"],
        serde_json::json!("audit_failed")
    );
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
}

fn finding<'a>(events: &'a [serde_json::Value], tool: &str) -> &'a serde_json::Value {
    events
        .iter()
        .find(|event| event["event"] == "diagnostic" && event["tool"] == tool)
        .unwrap_or_else(|| panic!("missing {tool} diagnostic"))
}

fn go_vulnerability(harness: &Harness) {
    write_go_mod(harness);
    write_advisory(harness, "go", GO_ADVISORY);
}

fn denied_license(harness: &Harness) {
    write_demo_license(harness, "AGPL-3.0-only");
}

#[test]
fn audit_json_emits_each_vulnerability_and_license_finding() {
    let (code, out, err) = run_with(
        &["security", "--output=json", "//third_party/go:go"],
        &AuditRunner::clean(),
        &go_vulnerability,
    );
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let vuln = finding(&events, "vuln");
    assert_eq!(vuln["rule"], serde_json::json!("vuln/GHSA-go-test-0001"));
    assert_eq!(vuln["path"], serde_json::json!("third_party/go/go.mod"));
    let message = vuln["message"].as_str().expect("message");
    assert!(
        message.contains("github.com/google/go-cmp@v0.6.0"),
        "{message}"
    );
    assert!(message.contains("GHSA-go-test-0001"), "{message}");
    assert!(message.contains("fixed in v0.7.0"), "{message}");
    assert_eq!(
        vuln["package"],
        serde_json::json!({"name": "github.com/google/go-cmp", "version": "v0.6.0", "fixed": ["v0.7.0"]})
    );
    assert_eq!(
        event(&events, "error")["code"],
        serde_json::json!("audit_failed")
    );
    assert_eq!(
        event(&events, "command_finished")["diagnostics"]["error"],
        serde_json::json!(1),
        "{out}"
    );

    let (code, out, err) = run_with(
        &[
            "license",
            "--output=json",
            "//rust/tests/fixtures/hello:demo",
        ],
        &AuditRunner::clean(),
        &denied_license,
    );
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let license = finding(&events, "license");
    assert_eq!(license["rule"], serde_json::json!("license/AGPL-3.0-only"));
    assert_eq!(
        license["path"],
        serde_json::json!("cc/context/Cargo.lock")
    );
    let message = license["message"].as_str().expect("message");
    assert!(
        message.starts_with("demo@1.0.0 license AGPL-3.0-only"),
        "{message}"
    );
    assert_eq!(
        license["package"],
        serde_json::json!({"name": "demo", "version": "1.0.0", "license": "AGPL-3.0-only"})
    );
    assert_eq!(
        event(&events, "error")["code"],
        serde_json::json!("audit_failed")
    );
}

#[test]
fn audit_text_prints_one_line_per_vulnerability_and_license_finding() {
    let (code, out, err) = run_with(
        &["security", "//third_party/go:go"],
        &AuditRunner::clean(),
        &go_vulnerability,
    );
    assert_eq!(code, 1, "{out}{err}");
    let line = out
        .lines()
        .find(|line| line.contains("GHSA-go-test-0001"))
        .unwrap_or_else(|| panic!("no vuln line: {out}"));
    assert!(line.starts_with("third_party/go/go.mod: error: "), "{line}");
    assert!(line.contains("github.com/google/go-cmp@v0.6.0"), "{line}");
    assert!(line.contains("fixed in v0.7.0"), "{line}");
    assert!(line.ends_with("[vuln/GHSA-go-test-0001]"), "{line}");

    let (code, out, err) = run_with(
        &[
            "license",
            "//rust/tests/fixtures/hello:demo",
            "--report=sarif=-",
        ],
        &AuditRunner::clean(),
        &denied_license,
    );
    assert_eq!(code, 1, "{out}{err}");
    serde_json::from_str::<serde_json::Value>(&out).expect("stdout stays SARIF");
    let line = err
        .lines()
        .find(|line| line.contains("AGPL-3.0-only"))
        .unwrap_or_else(|| panic!("no license line: {err}"));
    assert!(
        line.starts_with(
            "cc/context/Cargo.lock: error: demo@1.0.0 license AGPL-3.0-only"
        ),
        "{line}"
    );
}

#[test]
fn audit_reports_sarif_to_file() {
    use dx_plan::args::parse;
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-reports-cargo");
    write_hubs(&harness);
    write_cargo_license_set(&harness);
    let invocation = parse(&[
        "license".to_owned(),
        "//rust/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=out.sarif".to_owned(),
    ])
    .expect("parse");
    let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
    assert_eq!(runner.calls.borrow().len(), 0);
    assert_eq!(code, 0, "{out_text}{err_text}");
    let sarif = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    assert_eq!(value["version"], serde_json::json!("2.1.0"));
    assert_eq!(
        value["$schema"],
        serde_json::json!("https://json.schemastore.org/sarif-2.1.0.json")
    );
    let runs = value["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 1, "{value}");
    assert_eq!(
        runs[0]["tool"]["driver"]["name"],
        serde_json::json!("license")
    );
    assert_eq!(runs[0]["results"], serde_json::json!([]));
    assert_eq!(runs[0]["tool"]["driver"]["rules"], serde_json::json!([]));
    assert!(
        runs[0].get("invocations").is_none(),
        "complete run carries no unsuccessful invocation: {value}"
    );
    let typed: serde_sarif::sarif::Sarif = serde_json::from_str(&sarif).expect("typed SARIF");
    assert_eq!(typed.runs.len(), 1);
    assert_eq!(typed.runs[0].tool.driver.name, "license");
}

#[test]
fn license_spdx_report_is_valid_and_repeatable() {
    use dx_plan::args::parse;
    let runner = AuditRunner::clean();
    let mut documents = Vec::new();
    for _ in 0..2 {
        let harness = Harness::new("audit-reports-spdx");
        write_hubs(&harness);
        write_cargo_license_set(&harness);
        let invocation = parse(&[
            "license".to_owned(),
            "//rust/tests/fixtures/hello:hello".to_owned(),
            "--report=spdx=out.spdx.json".to_owned(),
        ])
        .expect("parse");
        let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
        assert_eq!(code, 0, "{out_text}{err_text}");
        documents
            .push(std::fs::read_to_string(harness.workspace.join("out.spdx.json")).expect("spdx"));
    }
    assert_eq!(documents[0], documents[1]);
    assert_eq!(
        dx_sbom_schema::spdx_errors(&documents[0]),
        Vec::<String>::new()
    );
    let value: serde_json::Value = serde_json::from_str(&documents[0]).expect("spdx JSON");
    let serde = value["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|package| package["name"] == "serde")
        .expect("serde package");
    assert_eq!(serde["versionInfo"], "1.0.100");
    assert_eq!(serde["licenseConcluded"], "MIT");
}

#[test]
fn license_writes_spdx_and_cyclonedx_from_one_run() {
    use dx_plan::args::parse;
    let runner = AuditRunner::clean();
    let mut documents = Vec::new();
    for _ in 0..2 {
        let harness = Harness::new("audit-reports-cyclonedx");
        write_hubs(&harness);
        write_cargo_license_set(&harness);
        let invocation = parse(&[
            "license".to_owned(),
            "//rust/tests/fixtures/hello:hello".to_owned(),
            "--report=spdx=out.spdx.json".to_owned(),
            "--report=cyclonedx=out.cdx.json".to_owned(),
        ])
        .expect("parse");
        let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
        assert_eq!(code, 0, "{out_text}{err_text}");
        let spdx = std::fs::read_to_string(harness.workspace.join("out.spdx.json")).expect("spdx");
        assert_eq!(dx_sbom_schema::spdx_errors(&spdx), Vec::<String>::new());
        documents.push(
            std::fs::read_to_string(harness.workspace.join("out.cdx.json")).expect("cyclonedx"),
        );
    }
    assert_eq!(documents[0], documents[1]);
    assert_eq!(
        dx_sbom_schema::cyclonedx_errors(&documents[0]),
        Vec::<String>::new()
    );
    let value: serde_json::Value = serde_json::from_str(&documents[0]).expect("cyclonedx JSON");
    let serde = value["components"]
        .as_array()
        .expect("components")
        .iter()
        .find(|component| component["name"] == "serde")
        .expect("serde component");
    assert_eq!(serde["version"], "1.0.100");
    assert_eq!(serde["licenses"][0]["license"]["id"], "MIT");
}

#[test]
fn security_rejects_the_spdx_report() {
    use dx_plan::args::parse;
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-reports-spdx-security");
    write_hubs(&harness);
    let invocation = parse(&["security".to_owned(), "--report=spdx=x".to_owned()]);
    match invocation {
        Err(error) => assert!(error.to_string().contains("spdx"), "{error}"),
        Ok(invocation) => {
            let (code, _, _) = harness.execute_with(&invocation, &runner);
            assert_ne!(code, 0);
        }
    }
}

#[test]
fn audit_sarif_run_shape_pins_family_tools_and_ordering() {
    use dx_plan::args::parse;
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-sarif-license-shape");
    write_hubs(&harness);
    write_cargo_license_set(&harness);
    let invocation = parse(&[
        "license".to_owned(),
        "//rust/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=out.sarif".to_owned(),
    ])
    .expect("parse");
    let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 0, "{out_text}{err_text}");
    let sarif = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    let names: Vec<&str> = value["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .map(|run| run["tool"]["driver"]["name"].as_str().expect("driver"))
        .collect();
    assert_eq!(names, vec!["license"]);
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-sarif-security-shape");
    write_hubs(&harness);
    write_all_lock_families_with_advisories(&harness);
    let invocation =
        parse(&["security".to_owned(), "--report=sarif=out.sarif".to_owned()]).expect("parse");
    let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 0, "{out_text}{err_text}");
    let sarif = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    assert_eq!(value["version"], serde_json::json!("2.1.0"));
    let runs = value["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 3, "{value}");
    assert_eq!(
        runs[0]["tool"]["driver"]["name"],
        serde_json::json!("gitleaks")
    );
    assert_eq!(
        runs[1]["tool"]["driver"]["name"],
        serde_json::json!("trivy")
    );
    assert_eq!(runs[2]["tool"]["driver"]["name"], serde_json::json!("vuln"));
    for run in runs {
        assert_eq!(run["results"], serde_json::json!([]), "{value}");
        assert!(run.get("invocations").is_none(), "complete: {value}");
    }
    let sarif_text = r#"{"version": "2.1.0", "runs": [{"tool": {"driver": {"name": "gitleaks"}}, "results": [{"ruleId": "gitleaks/aws-key", "message": {"text": "AWS key"}}]}]}"#;
    let runner = AuditRunner::with_sarif(Some(1), sarif_text);
    let (code, _out, _err) = run_with(&["security"], &runner, &|harness| {
        write_all_lock_families_with_advisories(harness);
    });
    assert_eq!(code, 1);
}

#[test]
fn audit_sarif_partial_marks_unsuccessful_while_retaining_findings() {
    use dx_plan::args::parse;
    let github = format!("{}{}", "ghp_", "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8");
    let sarif_in = format!(
        "{{\"version\": \"2.1.0\", \"runs\": [{{\"tool\": {{\"driver\": {{\"name\": \"gitleaks\"}}}}, \"results\": [{{\"ruleId\": \"gitleaks/aws-key\", \"message\": {{\"text\": \"leaked {github}\"}}, \"locations\": [{{\"physicalLocation\": {{\"artifactLocation\": {{\"uri\": \"src/app.py\"}}}}}}]}}]}}]}}"
    );
    let runner = AuditRunner::with_sarif(Some(1), &sarif_in);
    let harness = Harness::new("audit-sarif-partial");
    write_hubs(&harness);
    write_all_lock_families(&harness);
    let invocation =
        parse(&["security".to_owned(), "--report=sarif=out.sarif".to_owned()]).expect("parse");
    let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 1, "{out_text}{err_text}");
    let sarif = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    assert_eq!(value["version"], serde_json::json!("2.1.0"));
    let runs = value["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 3, "{value}");
    assert_eq!(
        runs[0]["tool"]["driver"]["name"],
        serde_json::json!("gitleaks")
    );
    assert_eq!(
        runs[1]["tool"]["driver"]["name"],
        serde_json::json!("trivy")
    );
    assert_eq!(runs[2]["tool"]["driver"]["name"], serde_json::json!("vuln"));
    let gitleaks_results = runs[0]["results"].as_array().expect("results");
    assert_eq!(gitleaks_results.len(), 1, "{value}");
    assert_eq!(
        gitleaks_results[0]["ruleId"],
        serde_json::json!("gitleaks/aws-key")
    );
    assert!(!sarif.contains(&github), "{sarif}");
    for run in runs {
        assert_eq!(
            run["invocations"],
            serde_json::json!([{"executionSuccessful": false}]),
            "partial run must mark unsuccessful: {value}"
        );
    }
    assert_eq!(
        runs[0]["tool"]["driver"]["rules"],
        serde_json::json!([{"id": "gitleaks/aws-key"}])
    );
    assert_eq!(
        gitleaks_results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        serde_json::json!("src/app.py")
    );
}

#[test]
fn audit_partial_reports_are_not_authoritative() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["security", "--output=json", "--report=sarif=out.sarif"],
        &runner,
        &|harness| {
            write_cargo_lock(harness);
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    let events = json_events(&out);
    let report = event(&events, "report");
    assert_eq!(report["format"], serde_json::json!("sarif"));
    assert_eq!(report["results_complete"], serde_json::json!(false));
    let finished = events.last().expect("finished");
    assert_eq!(finished["event"], serde_json::json!("command_finished"));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
    assert_eq!(finished["exit_code"], serde_json::json!(1));
}

#[test]
fn audit_errors_stay_typed_with_stable_display() {
    use super::AuditError;
    use std::error::Error as _;
    assert_eq!(
        AuditError::NoOwningSet {
            scope: "cc/tests/fixtures/adopt-cpp/solo:solo".to_owned(),
            sets: "cargo, go, npm".to_owned(),
        }
        .to_string(),
        "no owning dependency set for \"cc/tests/fixtures/adopt-cpp/solo:solo\"; audited sets: cargo, go, npm"
    );
    let read = AuditError::Read {
        rel: "pnpm-lock.yaml".to_owned(),
        error: std::io::Error::new(std::io::ErrorKind::NotFound, "boom"),
    };
    assert_eq!(read.to_string(), "could not read pnpm-lock.yaml: boom");
    assert!(read.source().is_some());
    assert_eq!(
        AuditError::NotUtf8 {
            rel: "pnpm-lock.yaml".to_owned(),
        }
        .to_string(),
        "could not read pnpm-lock.yaml: not valid UTF-8"
    );
    assert_eq!(
        AuditError::AdvisoryUnsupported {
            set: "cargo".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: unsupported set"
    );
    assert_eq!(
        AuditError::AdvisoryUtf8 {
            set: "cargo".to_owned(),
            rel: ".dx/local/advisory/cargo.json".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: .dx/local/advisory/cargo.json is not valid UTF-8"
    );
    assert_eq!(
        AuditError::AdvisoryParse {
            set: "cargo".to_owned(),
            detail: "boom".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: boom"
    );
    assert_eq!(
        AuditError::LockMissing {
            rel: "pnpm-lock.yaml".to_owned(),
        }
        .to_string(),
        "could not read pnpm-lock.yaml: no such file"
    );
    assert_eq!(
        AuditError::LockParse {
            rel: "pnpm-lock.yaml".to_owned(),
            detail: "boom".to_owned(),
        }
        .to_string(),
        "could not parse pnpm-lock.yaml: boom"
    );
    let security = AuditError::SecurityInvalid(dx_audit::exception::SecurityProblem::InvalidToml {
        message: "schema_version is missing".to_owned(),
    });
    assert_eq!(
        security.to_string(),
        "security.toml is invalid: schema_version is missing"
    );
    let harness = Harness::new("audit-typed-missing");
    let missing = super::load_advisories(&harness.workspace, "cargo", "cargo", false)
        .expect_err("missing pin fails");
    assert!(matches!(missing, AuditError::AdvisoryPinMissing { .. }));
    assert!(missing.to_string().contains("advisory_pin_missing"));
    let lock_missing = AuditError::LockMissing {
        rel: "Cargo.lock".to_owned(),
    };
    assert_eq!(
        missing.code(),
        dx_audit::advisory::CODE_ADVISORY_PIN_MISSING
    );
    assert_eq!(lock_missing.code(), "audit_failed");
    assert_eq!(security.code(), "audit_failed");
    assert_eq!(
        AuditError::OfflineRequired {
            set: "cargo".to_owned(),
            detail: missing.to_string(),
        }
        .code(),
        "offline_required"
    );
}

#[test]
fn audit_sarif_write_failures_are_fail_closed() {
    use dx_plan::args::parse;
    {
        let (format, path) = ("sarif", "missing-dir/out.sarif");
        let runner = AuditRunner::clean();
        let harness = Harness::new(&format!("audit-report-fail-{format}"));
        write_hubs(&harness);
        write_cargo_license_set(&harness);
        let invocation = parse(&[
            "license".to_owned(),
            "//rust/tests/fixtures/hello:hello".to_owned(),
            format!("--report={format}={path}"),
        ])
        .expect("parse");
        let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
        assert_eq!(code, 1, "{out_text}{err_text} {format}");
        assert!(err_text.contains("report_failed"), "{err_text} {format}");
        assert!(
            !harness.workspace.join("missing-dir").exists(),
            "missing parent is never created: {format}"
        );
    }
}

#[test]
fn audit_report_write_failure_json_reports_error_event() {
    use dx_plan::args::parse;
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-report-fail-json");
    write_hubs(&harness);
    write_cargo_license_set(&harness);
    let invocation = parse(&[
        "license".to_owned(),
        "//rust/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=missing-dir/out.sarif".to_owned(),
        "--output=json".to_owned(),
    ])
    .expect("parse");
    let (code, out_text, err_text) = harness.execute_with(&invocation, &runner);
    assert_eq!(code, 1, "{out_text}{err_text}");
    assert!(out_text.contains("report_failed"), "{out_text}");
    assert!(out_text.contains("command_finished"), "{out_text}");
    assert!(out_text.contains("results_complete"), "{out_text}");
}

#[test]
fn offline_live_missing_advisory_fails_with_offline_required() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(&["security", "--offline"], &runner, &|harness| {
        write_cargo_lock(harness);
    });
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("offline_required"), "{err}");
    assert!(err.contains("cannot obtain current advisory data"), "{err}");
    assert!(err.contains("dx: offline_required:"), "{err}");
    let (code, out, err) = run_with(
        &["security", "--offline", "--output=json"],
        &AuditRunner::clean(),
        &|harness| {
            write_cargo_lock(harness);
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"code\":\"offline_required\""), "{out}");
}
