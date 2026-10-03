use super::super::test_support::*;
use super::audit_live::*;

#[test]
fn secrets_launch_failure_is_incomplete_without_a_clean_result() {
    struct Unavailable;
    impl dx_process::Runner for Unavailable {
        fn gitleaks_tool(&self) -> Option<std::path::PathBuf> {
            Some(std::path::PathBuf::from(hermetic_gitleaks()))
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
    let (findings, incomplete, diagnostics) =
        super::run_secrets(&harness.workspace, &Unavailable, &harness.temp, 1, 0, false);
    assert!(findings.is_empty());
    assert!(diagnostics.is_empty());
    assert!(incomplete
        .expect("incomplete")
        .contains("failed to launch secrets auditor"));
}

#[test]
fn license_policy_read_failures_and_empty_default_are_explicit() {
    let harness = Harness::new("license-policy-io");
    let path = harness.workspace.join("licenses.toml");
    std::fs::write(&path, " \n").expect("empty");
    assert!(super::load_license_policy(&harness.workspace)
        .expect("default")
        .tables
        .allow
        .contains("MIT"));
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

fn npm_lock_with(license: &str) -> String {
    serde_json::json!({
        "name": "root",
        "lockfileVersion": 3,
        "packages": {
            "": {"name": "root"},
            "node_modules/demo": {"version": "1.0.0", "license": license}
        }
    })
    .to_string()
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
            &["license", "//javascript:demo", "--report=sarif=-"],
            &AuditRunner::clean(),
            &|harness| {
                harness.write_source("package-lock.json", &npm_lock_with(license));
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
fn an_internal_license_scope_skips_the_review_and_notice_tables() {
    let policy = "[policy]\nblocked = [\"AGPL-3.0-only\"]\n[distribution]\ninternal = [\"//javascript:internal\"]\n";
    for (license, code) in [("Zlib", 0), ("ISC", 0), ("MIT", 0), ("AGPL-3.0-only", 1)] {
        let (code_seen, out, err) = run_with(
            &["license", "//javascript:internal", "--report=sarif=-"],
            &AuditRunner::clean(),
            &|harness| {
                harness.write_source("package-lock.json", &npm_lock_with(license));
                harness.write_source("licenses.toml", policy);
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
            &["license", "//javascript:distributed", "--report=sarif=-"],
            &AuditRunner::clean(),
            &|harness| {
                harness.write_source("package-lock.json", &npm_lock_with(license));
                harness.write_source("licenses.toml", policy);
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
fn security_policy_read_failures_and_absent_file_are_explicit() {
    let harness = Harness::new("security-policy-io");
    assert!(super::load_security_policy(&harness.workspace)
        .expect("absent file")
        .is_empty());
    let path = harness.workspace.join("security.toml");
    std::fs::write(&path, " \n").expect("empty");
    assert!(super::load_security_policy(&harness.workspace)
        .expect("empty file")
        .is_empty());
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
        ("GPL-3.0-only", "[[exception]]\npackage = \"demo\"\nset = \"npm\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\nreason = \"reviewed\"\nexpires = \"2999-01-01\"\n", 0),
        ("GPL-3.0-only", "[[exception]]\npackage = \"other\"\nset = \"npm\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\nreason = \"reviewed\"\nexpires = \"2999-01-01\"\n", 1),
        ("GPL-3.0-only", "[[exception]]\npackage = \"demo\"\nset = \"npm\"\nlicense = \"GPL-3.0-only\"\nversions = \"1.0.0\"\nreason = \"reviewed\"\nexpires = \"2000-01-01\"\n", 1),
    ] {
        let (code, out, err) = run_with(&["license", "//javascript:demo", "--output=json"], &AuditRunner::clean(), &|h| {
            h.write_source("package-lock.json", &serde_json::json!({"packages":{"node_modules/demo":{"version":"1.0.0","license":license}}}).to_string());
            h.write_source("licenses.toml", &format!("[policy]\nblocked = [\"AGPL-3.0-only\"]\n[policy.distributed]\ndeny = [\"GPL-3.0-only\"]\nreview = [\"MPL-2.0\"]\n{exception}\n[[inventory]]\npackage = \"demo\"\nset = \"npm\"\nlicense = \"{license}\"\nversions = \"1.0.0\"\ntext_present = true\n"));
        });
        assert_eq!(code, expected, "{license}: {out}{err}");
    }
    let (code, out, err) = run_with(
        &["license", "//javascript:demo"],
        &AuditRunner::clean(),
        &|h| {
            h.write_source("package-lock.json", "{");
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("could not parse package-lock.json"));
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
fn audit_stdout_reports_are_single_machine_documents() {
    for (family, format) in [("security", "sarif"), ("license", "spdx")] {
        let report = format!("--report={format}=-");
        let (code, out, err) = run_with(&[family, &report], &AuditRunner::clean(), &|h| {
            clean_workspace_with_advisories(h);
        });
        assert_eq!(code, if family == "license" { 1 } else { 0 }, "{out}{err}");
        let value: serde_json::Value = serde_json::from_str(&out).expect("one JSON document");
        if format == "sarif" {
            assert_eq!(value["version"], "2.1.0");
        } else {
            assert_eq!(value["spdxVersion"], "SPDX-2.3");
        }
    }
}

#[test]
fn committed_gitleaks_config_produces_a_trust_warning() {
    for config in [".gitleaks.toml"] {
        let runner = AuditRunner::with_sarif(
            Some(0),
            r#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"gitleaks"}},"results":[]}]}"#,
        );
        let (code, out, err) = run_with(&["security", "--report=sarif=-"], &runner, &|h| {
            clean_workspace_with_advisories(h);
            h.write_source(config, "title = \"local rules\"\n");
        });
        assert_eq!(code, 0, "{out}{err}");
        let value: serde_json::Value = serde_json::from_str(&out).expect("sarif");
        assert!(value.to_string().contains("without a hash pin"));
        assert!(runner
            .calls
            .borrow()
            .iter()
            .any(|args| args.iter().any(|arg| arg.ends_with(config))));
    }
}

#[test]
fn advisory_loader_rejects_corrupt_snapshots_and_identity_sidecars() {
    use super::AuditError;
    use dx_update::sets::SetId;
    for variant in [
        "directory",
        "utf8",
        "empty",
        "missing-meta",
        "meta-utf8",
        "meta-json",
        "meta-invalid",
        "wrong-set",
        "invalid-json",
    ] {
        let harness = Harness::new(&format!("advisory-corrupt-{variant}"));
        write_advisory(&harness, "cargo", "[]");
        let snapshot = harness.workspace.join(".dx/advisory/cargo.json");
        let meta = harness.workspace.join(".dx/advisory/cargo.meta.json");
        match variant {
            "directory" => {
                std::fs::remove_file(&snapshot).expect("remove snapshot");
                std::fs::create_dir(&snapshot).expect("directory snapshot");
            }
            "utf8" => std::fs::write(&snapshot, [0xff]).expect("invalid utf8"),
            "empty" => std::fs::write(&snapshot, " \n").expect("empty snapshot"),
            "missing-meta" => std::fs::remove_file(&meta).expect("remove meta"),
            "meta-utf8" => std::fs::write(&meta, [0xff]).expect("invalid meta utf8"),
            "meta-json" => std::fs::write(&meta, "{").expect("invalid meta json"),
            "meta-invalid" | "wrong-set" => {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&meta).expect("meta")).expect("json");
                if variant == "meta-invalid" {
                    value["sha256"] = serde_json::json!("invalid");
                } else {
                    value["set"] = serde_json::json!("npm");
                    value["path"] = serde_json::json!(".dx/advisory/npm.json");
                    value["url"] = serde_json::json!(
                        dx_audit::advisory::advisory_source("npm").expect("source")
                    );
                }
                std::fs::write(&meta, value.to_string()).expect("write meta");
            }
            "invalid-json" => write_advisory(&harness, "cargo", "{"),
            _ => unreachable!(),
        }
        let error = super::load_advisories(&harness.workspace, SetId::Cargo, &super::today_utc())
            .expect_err(variant);
        let expected = match variant {
            "directory" => matches!(error, AuditError::AdvisoryRead { .. }),
            "utf8" => matches!(error, AuditError::AdvisoryUtf8 { .. }),
            "empty" => matches!(error, AuditError::AdvisoryEmpty { .. }),
            "missing-meta" => matches!(error, AuditError::AdvisoryMissingMeta { .. }),
            "meta-utf8" => matches!(error, AuditError::AdvisoryMeta { .. }),
            "meta-json" => matches!(error, AuditError::AdvisoryIdentity { .. }),
            "meta-invalid" => matches!(error, AuditError::AdvisorySnapshotInvalid { .. }),
            "wrong-set" => matches!(error, AuditError::AdvisorySetMismatch { .. }),
            "invalid-json" => matches!(error, AuditError::AdvisoryParse { .. }),
            _ => unreachable!(),
        };
        assert!(expected, "{variant}: {error}");
        assert!(error.to_string().starts_with("advisory_refresh_failed:"));
    }
}

#[test]
fn lock_loading_requires_readable_inputs_and_deduplicates_npm_siblings() {
    use super::AuditError;
    use dx_update::sets::SetId;
    let harness = Harness::new("audit-lock-loading");
    assert!(matches!(
        super::lock_texts_for_set(&harness.workspace, SetId::Npm),
        Err(AuditError::LockMissing { .. })
    ));
    harness.write_source("pnpm-lock.yaml", "packages:\n  demo@1.0.0: {}\n");
    harness.write_source(
        "package-lock.json",
        r#"{"packages":{"node_modules/demo":{"version":"1.0.0"}}}"#,
    );
    harness.write_source("yarn.lock", "demo@^1.0.0:\n  version \"1.0.0\"\n");
    let texts = super::lock_texts_for_set(&harness.workspace, SetId::Npm).expect("three locks");
    assert_eq!(texts.len(), 3);
    let (packages, paths) = super::parse_locked_for_set(SetId::Npm, &texts).expect("deduplicate");
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "demo");
    assert_eq!(paths.of("demo", "1.0.0"), "pnpm-lock.yaml");
    assert_eq!(paths.of("absent", "9.9.9"), "pnpm-lock.yaml");
    assert!(matches!(
        super::parse_locked_for_set(
            SetId::Npm,
            &[("package-lock.json".to_owned(), "{".to_owned())]
        ),
        Err(AuditError::LockParse { .. })
    ));
    std::fs::write(harness.workspace.join("package-lock.json"), [0xff]).expect("bad utf8");
    assert!(matches!(
        super::lock_texts_for_set(&harness.workspace, SetId::Npm),
        Err(AuditError::NotUtf8 { .. })
    ));
}

#[test]
fn audit_live_names_the_lockfile_each_finding_came_from() {
    use crate::args::parse;
    use crate::exec::{execute, Env};
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
        {"id": "GHSA-bbbb", "package": "jsononly", "versions": "2.0.0", "severity": "high", "fixed": [], "set": "npm"},
        {"id": "GHSA-cccc", "package": "yarnonly", "versions": "3.0.0", "severity": "high", "fixed": ["3.0.1"], "set": "npm"},
    ])
    .to_string();
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-lock-paths");
    harness.write_source(
        "pnpm-lock.yaml",
        "lockfileVersion: '9.0'\n\npackages:\n\n  'pnpmonly@1.0.0':\n    resolution: {integrity: sha512-abc}\n",
    );
    harness.write_source(
        "package-lock.json",
        r#"{"packages":{"node_modules/jsononly":{"version":"2.0.0","license":"GPL-3.0-only"}}}"#,
    );
    harness.write_source("yarn.lock", "yarnonly@^3.0.0:\n  version \"3.0.0\"\n");
    harness.write_source(
        "licenses.toml",
        "[policy]\nblocked = [\"AGPL-3.0-only\"]\n[policy.distributed]\ndeny = [\"GPL-3.0-only\"]\nreview = []\n[[inventory]]\npackage = \"pnpmonly\"\nset = \"npm\"\nlicense = \"AGPL-3.0-only\"\nversions = \"1.0.0\"\ntext_present = false\n[[inventory]]\npackage = \"yarnonly\"\nset = \"npm\"\nlicense = \"AGPL-3.0-only\"\nversions = \"3.0.0\"\ntext_present = false\n",
    );
    write_advisory(&harness, "npm", &advisories);
    for (command, report, driver) in [
        ("security", "vuln.sarif", "vuln"),
        ("license", "license.sarif", "license"),
    ] {
        let invocation = parse(&[
            command.to_owned(),
            "//javascript:demo".to_owned(),
            format!("--report=sarif={report}"),
        ])
        .expect("parse");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(
            &invocation,
            Env {
                workspace: &harness.workspace,
                runner: &runner,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        let out_text = String::from_utf8(out).expect("stdout");
        let err_text = String::from_utf8(err).expect("stderr");
        assert_eq!(code, 1, "{command}: {out_text}{err_text}");
        let by_package = sarif_paths(&harness, report, driver);
        assert_eq!(
            by_package.get("pnpmonly").map(String::as_str),
            Some("pnpm-lock.yaml"),
            "{command}: {by_package:?}"
        );
        assert_eq!(
            by_package.get("jsononly").map(String::as_str),
            Some("package-lock.json"),
            "{command}: {by_package:?}"
        );
        assert_eq!(
            by_package.get("yarnonly").map(String::as_str),
            Some("yarn.lock"),
            "{command}: {by_package:?}"
        );
    }
}

#[test]
fn audit_live_names_the_gemfile_lock_each_ruby_finding_came_from() {
    use crate::args::parse;
    use crate::exec::{execute, Env};
    let harness = Harness::new("audit-ruby-lock-paths");
    harness.write_source(
        "third_party/ruby/Gemfile.lock",
        "GEM\n  remote: https://rubygems.org/\n  specs:\n    rspec-core (3.13.0)\n",
    );
    harness.write_source(
        "examples/adopt-ruby/Gemfile.lock",
        "GEM\n  remote: https://rubygems.org/\n  specs:\n    rake (13.2.1)\n",
    );
    write_advisory(
        &harness,
        "rubygems",
        r#"[{"id":"GHSA-ruby-aaaa","package":"rspec-core","versions":">=3.0.0, <3.13.1","severity":"high","fixed":["3.13.1"],"set":"ruby"},{"id":"GHSA-ruby-bbbb","package":"rake","versions":"13.2.1","severity":"moderate","fixed":[],"set":"ruby"}]"#,
    );
    let invocation = parse(&[
        "security".to_owned(),
        "//ruby/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=vuln.sarif".to_owned(),
    ])
    .expect("parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &AuditRunner::clean(),
            query_runner: &harness.query,
            temp_dir: &harness.temp,
            pid: std::process::id(),
            nonce: 0,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    let out_text = String::from_utf8(out).expect("stdout");
    let err_text = String::from_utf8(err).expect("stderr");
    assert_eq!(code, 1, "{out_text}{err_text}");
    let sarif = std::fs::read_to_string(harness.workspace.join("vuln.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    let run = value["runs"]
        .as_array()
        .expect("runs")
        .iter()
        .find(|run| run["tool"]["driver"]["name"] == serde_json::json!("vuln"))
        .expect("vuln run");
    let mut by_package: std::collections::BTreeMap<String, String> = Default::default();
    for result in run["results"].as_array().expect("results") {
        let message = result["message"]["text"].as_str().expect("message");
        by_package.insert(
            message.split('@').next().expect("package").to_owned(),
            result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .expect("uri")
                .to_owned(),
        );
    }
    assert_eq!(
        by_package.get("rspec-core").map(String::as_str),
        Some("third_party/ruby/Gemfile.lock"),
        "{by_package:?}"
    );
    assert_eq!(
        by_package.get("rake").map(String::as_str),
        Some("examples/adopt-ruby/Gemfile.lock"),
        "{by_package:?}"
    );
    assert_eq!(by_package.len(), 2, "{by_package:?}");
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
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
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
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(kinds[0], "command_started");
    assert_eq!(kinds[kinds.len() - 1], "command_finished");
    assert!(kinds.contains(&"error"));
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(1)
    );
}

#[test]
fn audit_dry_run_json_emits_lifecycle() {
    let harness = Harness::new("audit-dryrun-json");
    let (code, out, err) = harness.run(&["security", "--dry-run", "--output=json"]);
    assert_eq!(code, 0, "{out}{err}");
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let kinds: Vec<&str> = events
        .iter()
        .map(|event| event["event"].as_str().expect("event"))
        .collect();
    assert_eq!(kinds, vec!["command_started", "command_finished"]);
    assert_eq!(
        events.last().expect("finished")["exit_code"],
        serde_json::json!(0)
    );
    assert!(
        harness.seen_env.borrow().is_empty(),
        "dry-run launches nothing"
    );
}

#[test]
fn audit_update_dry_run_quiet_prints_nothing() {
    for argv in [
        vec!["security", "--dry-run", "--quiet"],
        vec!["license", "--dry-run", "--quiet"],
        vec!["update", "--dry-run", "--quiet"],
    ] {
        let name = format!("dryrun-quiet-{}", argv[0]);
        let harness = Harness::new(&name);
        let (code, out, err) = harness.run(&argv);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, "", "{out}");
        assert_eq!(err, "", "{err}");
        assert!(
            harness.seen_env.borrow().is_empty(),
            "dry-run launches nothing"
        );
    }
}

#[test]
fn audit_reports_sarif_and_spdx_to_files() {
    use crate::args::parse;
    use crate::exec::{execute, Env};
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-reports-cargo");
    write_cargo_license_set(&harness);
    let invocation = parse(&[
        "license".to_owned(),
        "//rust/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=out.sarif".to_owned(),
        "--report=spdx=out.spdx.json".to_owned(),
    ])
    .expect("parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &runner,
            query_runner: &harness.query,
            temp_dir: &harness.temp,
            pid: std::process::id(),
            nonce: 0,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    assert_eq!(runner.calls.borrow().len(), 0);
    let out_text = String::from_utf8(out).expect("stdout");
    let err_text = String::from_utf8(err).expect("stderr");
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
    let spdx = std::fs::read_to_string(harness.workspace.join("out.spdx.json")).expect("spdx");
    let spdx_value: serde_json::Value = serde_json::from_str(&spdx).expect("spdx JSON");
    assert_eq!(spdx_value["spdxVersion"], serde_json::json!("SPDX-2.3"));
    assert_eq!(spdx_value["dataLicense"], serde_json::json!("CC0-1.0"));
    assert_eq!(spdx_value["SPDXID"], serde_json::json!("SPDXRef-DOCUMENT"));
    assert_eq!(spdx_value["name"], serde_json::json!("dx-audit-license"));
    let namespace = spdx_value["documentNamespace"].as_str().expect("namespace");
    assert!(
        namespace.starts_with("https://dx-audit.local/"),
        "invocation-unique namespace: {namespace}"
    );
    let packages = spdx_value["packages"].as_array().expect("packages");
    assert_eq!(packages.len(), 1, "{spdx_value}");
    assert_eq!(
        packages[0]["SPDXID"],
        serde_json::json!("SPDXRef-Package-1")
    );
    assert_eq!(packages[0]["name"], serde_json::json!("serde"));
    assert_eq!(packages[0]["versionInfo"], serde_json::json!("1.0.100"));
    assert_eq!(packages[0]["licenseConcluded"], serde_json::json!("MIT"));
    assert_eq!(packages[0]["licenseDeclared"], serde_json::json!("MIT"));
    assert_eq!(
        packages[0]["copyrightText"],
        serde_json::json!("NOASSERTION")
    );
    assert_eq!(
        packages[0]["externalRefs"],
        serde_json::json!([{
            "referenceCategory": "PACKAGE-MANAGER",
            "referenceType": "purl",
            "referenceLocator": "pkg:cargo/serde@1.0.100",
        }]),
        "{spdx_value}"
    );
    let rels = spdx_value["relationships"]
        .as_array()
        .expect("relationships");
    assert_eq!(
        rels,
        &vec![serde_json::json!({
            "spdxElementId": "//rust/tests/fixtures/hello:hello",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": "SPDXRef-DOCUMENT",
        })],
        "one DESCRIBES per audited root, no V1 CONTAINS: {spdx_value}"
    );
}

#[test]
fn audit_sarif_run_shape_pins_family_tools_and_ordering() {
    use crate::args::parse;
    use crate::exec::{execute, Env};
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-sarif-license-shape");
    write_cargo_license_set(&harness);
    let invocation = parse(&[
        "license".to_owned(),
        "//rust/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=out.sarif".to_owned(),
    ])
    .expect("parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &runner,
            query_runner: &harness.query,
            temp_dir: &harness.temp,
            pid: std::process::id(),
            nonce: 0,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    assert_eq!(
        code,
        0,
        "{}{}",
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap()
    );
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
    write_all_lock_families_with_advisories(&harness);
    let invocation =
        parse(&["security".to_owned(), "--report=sarif=out.sarif".to_owned()]).expect("parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &runner,
            query_runner: &harness.query,
            temp_dir: &harness.temp,
            pid: std::process::id(),
            nonce: 1,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    assert_eq!(
        code,
        0,
        "{}{}",
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap()
    );
    let sarif = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    assert_eq!(value["version"], serde_json::json!("2.1.0"));
    let runs = value["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 2, "{value}");
    assert_eq!(
        runs[0]["tool"]["driver"]["name"],
        serde_json::json!("gitleaks")
    );
    assert_eq!(runs[1]["tool"]["driver"]["name"], serde_json::json!("vuln"));
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
    use crate::args::parse;
    use crate::exec::{execute, Env};
    let github = format!("{}{}", "ghp_", "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8");
    let sarif_in = format!(
        "{{\"version\": \"2.1.0\", \"runs\": [{{\"tool\": {{\"driver\": {{\"name\": \"gitleaks\"}}}}, \"results\": [{{\"ruleId\": \"gitleaks/aws-key\", \"message\": {{\"text\": \"leaked {github}\"}}, \"locations\": [{{\"physicalLocation\": {{\"artifactLocation\": {{\"uri\": \"src/app.py\"}}}}}}]}}]}}]}}"
    );
    let runner = AuditRunner::with_sarif(Some(1), &sarif_in);
    let harness = Harness::new("audit-sarif-partial");
    write_all_lock_families(&harness);
    let invocation =
        parse(&["security".to_owned(), "--report=sarif=out.sarif".to_owned()]).expect("parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &runner,
            query_runner: &harness.query,
            temp_dir: &harness.temp,
            pid: std::process::id(),
            nonce: 2,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    assert_eq!(
        code,
        1,
        "{}{}",
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap()
    );
    let sarif = std::fs::read_to_string(harness.workspace.join("out.sarif")).expect("sarif");
    let value: serde_json::Value = serde_json::from_str(&sarif).expect("sarif JSON");
    assert_eq!(value["version"], serde_json::json!("2.1.0"));
    let runs = value["runs"].as_array().expect("runs");
    assert_eq!(runs.len(), 2, "{value}");
    assert_eq!(
        runs[0]["tool"]["driver"]["name"],
        serde_json::json!("gitleaks")
    );
    assert_eq!(runs[1]["tool"]["driver"]["name"], serde_json::json!("vuln"));
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
fn audit_spdx_live_golden_is_single_deterministic_document() {
    use crate::args::parse;
    use crate::exec::{execute, Env};
    fn emit(nonce: u64) -> (serde_json::Value, i32, String) {
        let runner = AuditRunner::clean();
        let harness = Harness::new(&format!("audit-spdx-determinism-{nonce}"));
        write_cargo_license_set(&harness);
        let invocation = parse(&[
            "license".to_owned(),
            "//rust/tests/fixtures/hello:hello".to_owned(),
            "--report=spdx=out.spdx.json".to_owned(),
        ])
        .expect("parse");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(
            &invocation,
            Env {
                workspace: &harness.workspace,
                runner: &runner,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: 999,
                nonce,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        let text = std::fs::read_to_string(harness.workspace.join("out.spdx.json")).expect("spdx");
        let mut value: serde_json::Value = serde_json::from_str(&text).expect("spdx JSON");
        value["documentNamespace"] = serde_json::json!("NORMALIZED");
        (value, code, text)
    }
    let (first, first_code, first_text) = emit(10);
    let (second, second_code, second_text) = emit(11);
    assert_eq!(first_code, 0);
    assert_eq!(second_code, 0);
    assert_eq!(first, second, "packages plus relationships deterministic");
    assert_ne!(first_text, second_text);
    let first_ns = serde_json::from_str::<serde_json::Value>(&first_text).expect("json")
        ["documentNamespace"]
        .as_str()
        .expect("ns")
        .to_owned();
    let second_ns = serde_json::from_str::<serde_json::Value>(&second_text).expect("json")
        ["documentNamespace"]
        .as_str()
        .expect("ns")
        .to_owned();
    assert!(
        first_ns.starts_with("https://dx-audit.local/"),
        "{first_ns}"
    );
    assert!(
        second_ns.starts_with("https://dx-audit.local/"),
        "{second_ns}"
    );
    assert_ne!(first_ns, second_ns);
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
    let events: Vec<serde_json::Value> = out
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("NDJSON");
    let report = events
        .iter()
        .find(|event| event["event"] == serde_json::json!("report"))
        .expect("report event");
    assert_eq!(report["format"], serde_json::json!("sarif"));
    assert_eq!(report["results_complete"], serde_json::json!(false));
    let finished = events.last().expect("finished");
    assert_eq!(finished["event"], serde_json::json!("command_finished"));
    assert_eq!(finished["results_complete"], serde_json::json!(false));
    assert_eq!(finished["exit_code"], serde_json::json!(1));
}

#[test]
fn audit_set_coverage_is_decided_once_and_matches_the_set_registry() {
    let names: Vec<&str> = dx_update::sets::SetId::ALL
        .iter()
        .map(|set| set.name())
        .collect();
    for name in &names {
        assert_eq!(
            dx_audit::backend::is_empty_set(name),
            dx_audit::advisory::advisory_source(name).is_none(),
            "{name} needs lock coverage and an advisory source together"
        );
    }
    let covered: std::collections::BTreeSet<&str> = names
        .iter()
        .copied()
        .filter(|name| !dx_audit::backend::is_empty_set(name))
        .collect();
    let curated: std::collections::BTreeSet<&str> = dx_audit::curator::CURATOR_ADVISORY_SETS
        .iter()
        .copied()
        .collect();
    let covered_families: std::collections::BTreeSet<&str> = covered
        .iter()
        .filter_map(|name| dx_audit::advisory::advisory_family(name))
        .collect();
    assert_eq!(curated, covered_families);
    let sibling_format_sets = ["npm"];
    for set in dx_update::sets::SetId::ALL {
        let name = set.name();
        if sibling_format_sets.contains(&name) {
            continue;
        }
        for rel in dx_audit::backend::vuln_locks(name) {
            assert!(
                set.locks().contains(rel),
                "{name} is audited against {rel}, which its updater set does not own"
            );
        }
    }
    let uncovered = dx_audit::backend::sets_without_coverage(names.iter().copied());
    assert_eq!(
        uncovered,
        names
            .iter()
            .copied()
            .filter(|name| !covered.contains(name))
            .collect::<Vec<&str>>()
    );
    assert!(
        !uncovered.is_empty(),
        "every uncovered set must be reported"
    );
}

#[test]
fn audit_errors_stay_typed_with_stable_display() {
    use super::AuditError;
    use std::error::Error as _;
    assert_eq!(
        AuditError::NoOwningSet {
            scope: "examples/adopt-cpp/solo:solo".to_owned(),
            sets: super::audited_set_names().join(", "),
        }
        .to_string(),
        "no owning dependency set for \"examples/adopt-cpp/solo:solo\"; audited sets: cargo, go, maven, npm, npm-adopt, npm-adopt-polyglot, npm-tools, nuget, powershell, ruby"
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
        AuditError::AdvisoryMissing {
            set: "cargo".to_owned(),
            rel: ".dx/advisory/cargo.json".to_owned(),
            upstream: "https://example.invalid/cargo.zip".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: missing .dx/advisory/cargo.json (refresh via https://example.invalid/cargo.zip)"
    );
    let snapshot_read = AuditError::AdvisoryRead {
        set: "cargo".to_owned(),
        rel: ".dx/advisory/cargo.json".to_owned(),
        error: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    assert_eq!(
        snapshot_read.to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: could not read .dx/advisory/cargo.json: denied"
    );
    assert!(snapshot_read.source().is_some());
    assert_eq!(
        AuditError::AdvisoryUtf8 {
            set: "cargo".to_owned(),
            rel: ".dx/advisory/cargo.json".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: .dx/advisory/cargo.json is not valid UTF-8"
    );
    assert_eq!(
        AuditError::AdvisoryEmpty {
            set: "cargo".to_owned(),
            rel: ".dx/advisory/cargo.json".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: empty .dx/advisory/cargo.json"
    );
    assert_eq!(
        AuditError::AdvisoryMissingMeta {
            set: "cargo".to_owned(),
            meta_rel: ".dx/advisory/cargo.meta.json".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: missing .dx/advisory/cargo.meta.json"
    );
    let meta_wrapped = AuditError::AdvisoryMeta {
        set: "cargo".to_owned(),
        source: Box::new(AuditError::NotUtf8 {
            rel: ".dx/advisory/cargo.meta.json".to_owned(),
        }),
    };
    assert_eq!(
        meta_wrapped.to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: could not read .dx/advisory/cargo.meta.json: not valid UTF-8"
    );
    assert!(meta_wrapped.source().is_some());
    assert_eq!(
        AuditError::AdvisoryIdentity {
            set: "cargo".to_owned(),
            detail: "invalid advisory identity: boom".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: invalid advisory identity: boom"
    );
    assert_eq!(
        AuditError::AdvisorySetMismatch {
            set: "cargo".to_owned(),
            actual: "npm".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: identity set \"npm\" does not match"
    );
    assert_eq!(
        AuditError::AdvisoryStale {
            set: "cargo".to_owned(),
            retrieved_at: "2000-01-01".to_owned(),
            today: "2026-09-22".to_owned(),
            upstream: "https://example.invalid/cargo.zip".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: stale snapshot 2000-01-01 (want 2026-09-22; refresh via https://example.invalid/cargo.zip)"
    );
    assert_eq!(
        AuditError::AdvisoryShaMismatch {
            set: "cargo".to_owned(),
            rel: ".dx/advisory/cargo.json".to_owned(),
        }
        .to_string(),
        "advisory_refresh_failed: could not obtain current advisory data for cargo: identity sha256 does not match .dx/advisory/cargo.json"
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
    let security =
        AuditError::SecurityInvalid(dx_audit::exception::SecurityProblem::UnsupportedSchema {
            version: 99,
        });
    assert_eq!(
        security.to_string(),
        "unsupported security.toml schema_version 99 (want 1)"
    );
    let unowned = super::resolve_audit_sets(&["docs/cli/README.md".to_owned()])
        .expect_err("unowned scope fails");
    assert!(matches!(unowned, AuditError::NoOwningSet { .. }));
    assert!(unowned.to_string().contains("no owning dependency set"));
    let harness = Harness::new("audit-typed-missing");
    let missing = super::load_advisories(
        &harness.workspace,
        dx_update::sets::SetId::Cargo,
        "2026-09-22",
    )
    .expect_err("missing snapshot fails");
    assert!(matches!(missing, AuditError::AdvisoryMissing { .. }));
    assert!(missing.to_string().contains("advisory_refresh_failed"));
    let lock_missing = super::lock_texts_for_set(&harness.workspace, dx_update::sets::SetId::Cargo)
        .expect_err("missing lock fails");
    assert!(matches!(lock_missing, AuditError::LockMissing { .. }));
}

#[test]
fn audit_scopes_resolve_to_the_set_that_owns_the_pinned_dependency() {
    use super::AuditError;
    use dx_update::sets::SetId;
    for (scope, expected) in [
        ("//examples/adopt-csharp/greet:greet", vec![SetId::NuGet]),
        ("//examples/adopt-fsharp/solo:solo", vec![SetId::NuGet]),
        ("//examples/adopt-scala/greet:greet", vec![SetId::Maven]),
        ("//csharp/tests/fixtures/hello:hello", vec![SetId::NuGet]),
        ("//scala/greet:greet", vec![SetId::Maven]),
        ("//python/tests/fixtures/hello/hello.py", vec![SetId::Uv]),
        ("//ruby/tests/fixtures/hello:hello", vec![SetId::Ruby]),
        ("//examples/adopt-ruby/greet:greet", vec![SetId::Ruby]),
        (
            "//examples/adopt-powershell/greet:greet",
            vec![SetId::PowerShell],
        ),
        (
            "//third_party/powershell:PSGallery.lock.json",
            vec![SetId::PowerShell],
        ),
    ] {
        assert_eq!(
            super::resolve_audit_sets(&[scope.to_owned()]).expect(scope),
            expected,
            "{scope}"
        );
    }
    assert_eq!(
        super::audited_set_names(),
        vec![
            "cargo",
            "go",
            "maven",
            "npm",
            "npm-adopt",
            "npm-adopt-polyglot",
            "npm-tools",
            "nuget",
            "powershell",
            "ruby"
        ]
    );
    for scope in [
        "//examples/adopt-cpp/solo:solo",
        "//examples/adopt-cpp/greet:greet",
    ] {
        let unowned = super::resolve_audit_sets(&[scope.to_owned()]).expect_err(scope);
        assert!(
            matches!(unowned, AuditError::NoOwningSet { .. }),
            "{scope} has no dependency set"
        );
    }
    assert_eq!(
        super::resolve_audit_sets(&["//quality/...".to_owned()]).expect("quality tree"),
        vec![SetId::Cargo, SetId::NpmTools, SetId::UvTools],
        "a recursive scope must not skip the lockfiles nested under it"
    );
}

#[test]
fn audit_sarif_spdx_write_failures_are_fail_closed() {
    use crate::args::parse;
    use crate::exec::{execute, Env};
    for (format, path) in [
        ("sarif", "missing-dir/out.sarif"),
        ("spdx", "missing-dir/out.spdx.json"),
    ] {
        let runner = AuditRunner::clean();
        let harness = Harness::new(&format!("audit-report-fail-{format}"));
        write_cargo_license_set(&harness);
        let invocation = parse(&[
            "license".to_owned(),
            "//rust/tests/fixtures/hello:hello".to_owned(),
            format!("--report={format}={path}"),
        ])
        .expect("parse");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = execute(
            &invocation,
            Env {
                workspace: &harness.workspace,
                runner: &runner,
                query_runner: &harness.query,
                temp_dir: &harness.temp,
                pid: std::process::id(),
                nonce: 0,
                out: &mut out,
                err: &mut err,
                ci: false,
            },
        );
        let out_text = String::from_utf8(out).expect("stdout");
        let err_text = String::from_utf8(err).expect("stderr");
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
    use crate::args::parse;
    use crate::exec::{execute, Env};
    let runner = AuditRunner::clean();
    let harness = Harness::new("audit-report-fail-json");
    write_cargo_license_set(&harness);
    let invocation = parse(&[
        "license".to_owned(),
        "//rust/tests/fixtures/hello:hello".to_owned(),
        "--report=sarif=missing-dir/out.sarif".to_owned(),
        "--output=json".to_owned(),
    ])
    .expect("parse");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = execute(
        &invocation,
        Env {
            workspace: &harness.workspace,
            runner: &runner,
            query_runner: &harness.query,
            temp_dir: &harness.temp,
            pid: std::process::id(),
            nonce: 0,
            out: &mut out,
            err: &mut err,
            ci: false,
        },
    );
    let out_text = String::from_utf8(out).expect("stdout");
    let err_text = String::from_utf8(err).expect("stderr");
    assert_eq!(code, 1, "{out_text}{err_text}");
    assert!(out_text.contains("report_failed"), "{out_text}");
    assert!(out_text.contains("command_finished"), "{out_text}");
    assert!(out_text.contains("results_complete"), "{out_text}");
}

#[test]
fn offline_dry_run_plans_cache_only_without_launching() {
    let harness = Harness::new("audit-offline-dryrun");
    let (code, out, err) = harness.run(&["security", "--offline", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out, "Running audit security for //... (offline, cache-only)\n",
        "{out}"
    );
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "offline dry-run launches nothing"
    );
    let alias = Harness::new("audit-frozen-dryrun");
    let (code, out, err) = alias.run(&["license", "--frozen", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        out, "Running audit license for //... (offline, cache-only)\n",
        "{out}"
    );
    let online = Harness::new("audit-online-dryrun");
    let (code, out, err) = online.run(&["security", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "Running audit security for //...\n", "{out}");
    assert_eq!(err, "", "{err}");
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
