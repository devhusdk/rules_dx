use super::super::test_support::*;
use dx_process::{ChildStatus, Runner};
use std::cell::RefCell;
use std::io;
use std::path::Path;
use std::rc::Rc;

/// Returns the pinned gitleaks path the fakes hand to the backend.
pub(super) fn hermetic_gitleaks() -> String {
    if cfg!(windows) {
        r"C:\hermetic\gitleaks.exe".to_owned()
    } else {
        "/hermetic/gitleaks".to_owned()
    }
}

pub(super) struct AuditRunner {
    pub(super) calls: Rc<RefCell<Vec<Vec<String>>>>,
    pub(super) envs: Rc<RefCell<Vec<Vec<(String, String)>>>>,
    pub(super) code: Option<i32>,
    pub(super) sarif: Option<String>,
}

impl AuditRunner {
    pub(super) fn clean() -> Self {
        AuditRunner {
            calls: Rc::new(RefCell::new(Vec::new())),
            envs: Rc::new(RefCell::new(Vec::new())),
            code: Some(0),
            sarif: None,
        }
    }

    pub(super) fn with_sarif(code: Option<i32>, sarif: &str) -> Self {
        AuditRunner {
            calls: Rc::new(RefCell::new(Vec::new())),
            envs: Rc::new(RefCell::new(Vec::new())),
            code,
            sarif: Some(sarif.to_owned()),
        }
    }
}

impl Runner for AuditRunner {
    fn run(&self, argv: &[String], _cwd: &Path, env: &[(&str, &str)]) -> io::Result<ChildStatus> {
        self.calls.borrow_mut().push(argv.to_vec());
        self.envs.borrow_mut().push(
            env.iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        );
        if let Some(sarif) = &self.sarif {
            for (index, arg) in argv.iter().enumerate() {
                if arg == "--report-path" {
                    if let Some(path) = argv.get(index + 1) {
                        let _ = std::fs::write(path, sarif.as_bytes());
                    }
                }
            }
        }
        Ok(ChildStatus { code: self.code })
    }

    fn run_hermetic(
        &self,
        argv: &[String],
        cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<ChildStatus> {
        self.run(argv, cwd, env)
    }

    fn gitleaks_tool(&self) -> Option<std::path::PathBuf> {
        Some(std::path::PathBuf::from(hermetic_gitleaks()))
    }
}

pub(super) fn run_with(
    argv: &[&str],
    runner: &AuditRunner,
    setup: &dyn Fn(&Harness),
) -> (i32, String, String) {
    use crate::args::parse;
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
    let invocation = parse(&words).expect("parse");
    let harness = Harness::new(&format!(
        "audit-live-{}-{}-{}",
        std::thread::current()
            .name()
            .unwrap_or("test")
            .replace(':', "_"),
        argv.join("-").replace('/', "_"),
        id
    ));
    setup(&harness);
    harness.execute_with(&invocation, runner)
}

pub(super) fn clean_workspace(harness: &Harness) {
    write_all_lock_families(harness);
    write_cargo_license_lock(harness);
    write_mit_licenses(harness);
}

pub(super) fn clean_workspace_with_advisories(harness: &Harness) {
    clean_workspace(harness);
    write_all_empty_advisories(harness);
}

pub(super) fn write_all_lock_families(harness: &Harness) {
    write_cargo_lock(harness);
    write_npm_locks(harness);
    write_maven_lock(harness);
    write_nuget_lock(harness);
    write_go_mod(harness);
    write_ruby_locks(harness);
    write_powershell_locks(harness);
}

pub(super) fn write_all_lock_families_with_advisories(harness: &Harness) {
    write_all_lock_families(harness);
    write_all_empty_advisories(harness);
}

pub(super) fn write_cargo_license_set(harness: &Harness) {
    write_cargo_lock(harness);
    write_cargo_license_lock(harness);
    write_cargo_inventory_licenses(harness);
}

pub(super) fn write_cargo_lock(harness: &Harness) {
    harness.write_source(
        "rust/tests/fixtures/hello/Cargo.lock",
        "[[package]]\nname = \"serde\"\nversion = \"1.0.100\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
    );
}

pub(super) fn write_cargo_license_lock(harness: &Harness) {
    harness.write_source(
        "cargo-bazel-lock.json",
        r#"{"packages": {"serde 1.0.100": {"license": "MIT"}}}"#,
    );
}

pub(super) fn write_maven_lock(harness: &Harness) {
    harness.write_source("third_party/jvm/maven_install.json", r#"{"artifacts": {}}"#);
}

pub(super) fn write_nuget_lock(harness: &Harness) {
    harness.write_source(
        "third_party/dotnet/paket.lock",
        "NUGET\n  remote: https://api.nuget.org/v3/index.json\n",
    );
}

pub(super) fn write_mit_licenses(harness: &Harness) {
    harness.write_source(
        "licenses.toml",
        "[policy.distributed]\nallow = [\"MIT\"]\nreview = []\ndeny = []\n",
    );
}

pub(super) fn write_cargo_inventory_licenses(harness: &Harness) {
    harness.write_source(
        "licenses.toml",
        "[policy.distributed]\nallow = [\"MIT\"]\nreview = []\ndeny = []\n\n[[inventory]]\npackage = \"serde\"\nset = \"cargo\"\nlicense = \"MIT\"\nversions = \"1.0.100\"\ntext_present = true\n",
    );
}

pub(super) fn write_go_mod(harness: &Harness) {
    harness.write_source(
        "third_party/go/go.mod",
        "module rules_dx/third_party/go\n\ngo 1.24.12\n\nrequire (\n\tgithub.com/bazelbuild/buildtools v0.0.0-20250930140053-2eb4fccefb52 // indirect\n\tgithub.com/google/go-cmp v0.6.0\n\tgithub.com/pmezard/go-difflib v1.0.0\n)\n",
    );
}

pub(super) fn write_powershell_locks(harness: &Harness) {
    harness.write_source(
        "third_party/powershell/PSGallery.lock.json",
        "{\n  \"pwsh\": \"7.5.4\",\n  \"modules\": {\n    \"Pester\": {\n      \"version\": \"5.7.1\"\n    },\n    \"PSScriptAnalyzer\": {\n      \"version\": \"1.25.0\"\n    }\n  }\n}\n",
    );
}

pub(super) fn write_ruby_locks(harness: &Harness) {
    for rel in [
        "third_party/ruby/Gemfile.lock",
        "examples/adopt-ruby/Gemfile.lock",
    ] {
        harness.write_source(
            rel,
            "GEM\n  remote: https://rubygems.org/\n  specs:\n    rspec (3.13.0)\n      rspec-core (~> 3.13.0)\n    rspec-core (3.13.0)\n      rspec-support (~> 3.13.0)\n    rspec-support (3.13.1)\n",
        );
    }
}

const NPM_LOCK: &str =
    "lockfileVersion: '9.0'\n\npackages:\n\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n";

const NPM_FAMILY_LOCKS: [&str; 3] = [
    "examples/adopt-js-ts/pnpm-lock.yaml",
    "examples/adopt-polyglot/pnpm-lock.yaml",
    "quality/tools/javascript/pnpm-lock.yaml",
];

pub(super) fn write_npm_locks(harness: &Harness) {
    harness.write_source("pnpm-lock.yaml", NPM_LOCK);
    for rel in NPM_FAMILY_LOCKS {
        harness.write_source(rel, NPM_LOCK);
    }
}

pub(super) fn write_advisory(harness: &Harness, set: &str, json: &str) {
    let today = super::today_utc();
    let url = dx_audit::advisory::advisory_source(set)
        .expect("supported set needs a source")
        .to_owned();
    let sha = dx_digest::sha256_hex(json.as_bytes());
    harness.write_source(&format!(".dx/advisory/{set}.json"), json);
    let meta = serde_json::json!({
        "set": set,
        "url": url,
        "sha256": sha,
        "retrieved_at": today,
        "path": format!(".dx/advisory/{set}.json"),
    });
    harness.write_source(&format!(".dx/advisory/{set}.meta.json"), &meta.to_string());
}

pub(super) fn write_all_empty_advisories(harness: &Harness) {
    for set in ["cargo", "npm", "maven", "nuget", "go", "rubygems"] {
        write_advisory(harness, set, "[]");
    }
}

#[test]
pub(super) fn audit_dry_run_plans_families_without_launching() {
    let harness = Harness::new("audit-dryrun");
    let (code, out, err) = harness.run(&["security", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running audit security for //..."), "{out}");
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "dry-run launches nothing"
    );

    let harness = Harness::new("audit-dryrun-family");
    let (code, out, err) = harness.run(&["license", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running audit license for //..."), "{out}");
    assert_eq!(err, "", "{err}");
    assert!(
        harness.seen_env.borrow().is_empty(),
        "dry-run launches nothing"
    );
}

#[test]
pub(super) fn audit_live_clean_runs_gitleaks_and_exits_zero() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(&["security"], &runner, &|harness| {
        write_all_lock_families_with_advisories(harness);
    });
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Running audit security for //..."), "{out}");
    assert!(out.contains("audit security: clean"), "{out}");
    assert!(
        out.contains(
            "audit security: clean; no advisory coverage for uv, uv-adopt, uv-adopt-polyglot, \
             uv-tools"
        ),
        "{out}"
    );
    assert_eq!(err, "", "{err}");
    assert_eq!(runner.calls.borrow().len(), 1);
    assert_eq!(runner.calls.borrow()[0][0], hermetic_gitleaks());
    assert!(runner.calls.borrow()[0].contains(&"--redact".to_owned()));
    assert_eq!(runner.envs.borrow().len(), 1);
    assert_eq!(runner.envs.borrow()[0].len(), 1);
    assert_eq!(runner.envs.borrow()[0][0].0, "TMPDIR");
    assert!(!runner.envs.borrow()[0].iter().any(|(key, _)| key == "PATH"));
    assert!(!runner.envs.borrow()[0]
        .iter()
        .any(|(key, _)| key == "GITLEAKS_CONFIG"));
}

#[test]
pub(super) fn audit_live_secrets_findings_fail_with_redacted_summary() {
    let github = format!("{}{}", "ghp_", "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6q7r8");
    let generic = format!("{}{}", "sk-live-", "51H7x9yQ2wE4rT6yU8iO0p");
    let sarif = format!(
        "{{\"version\": \"2.1.0\", \"runs\": [{{\"tool\": {{\"driver\": {{\"name\": \"gitleaks\"}}}}, \"results\": [{{\"ruleId\": \"gitleaks/aws-key\", \"message\": {{\"text\": \"leaked AKIAIOSFODNN7EXAMPLE in src/app.py\"}}, \"fingerprints\": {{\"secret\": \"AKIAIOSFODNN7EXAMPLE\"}}, \"partialFingerprints\": {{\"secret/v1\": \"{github}\"}}, \"properties\": {{\"secret\": \"{generic}\"}}, \"locations\": [{{\"physicalLocation\": {{\"artifactLocation\": {{\"uri\": \"src/app.py\"}}, \"region\": {{\"snippet\": {{\"text\": \"key = 'AKIAIOSFODNN7EXAMPLE'\"}}}}}}}}]}}]}}]}}"
    );
    let runner = AuditRunner::with_sarif(Some(1), &sarif);
    let (code, out, err) = run_with(&["security"], &runner, &|harness| {
        write_all_lock_families_with_advisories(harness);
    });
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("audit security"), "{err}");
    for secret in ["AKIAIOSFODNN7EXAMPLE", github.as_str(), generic.as_str()] {
        assert!(!out.contains(secret), "{out}");
        assert!(!err.contains(secret), "{err}");
    }
    assert!(runner.calls.borrow()[0].contains(&"--redact".to_owned()));
}

#[test]
pub(super) fn audit_live_without_hermetic_tool_fails_closed() {
    struct NoToolRunner {
        calls: Rc<RefCell<Vec<Vec<String>>>>,
    }
    impl Runner for NoToolRunner {
        fn run(
            &self,
            argv: &[String],
            _cwd: &Path,
            _env: &[(&str, &str)],
        ) -> io::Result<ChildStatus> {
            self.calls.borrow_mut().push(argv.to_vec());
            Ok(ChildStatus { code: Some(0) })
        }
    }
    let runner = NoToolRunner {
        calls: Rc::new(RefCell::new(Vec::new())),
    };
    assert!(runner.gitleaks_tool().is_none());
    let (code, _out, err) = {
        use crate::args::parse;
        let words: Vec<String> = ["security"].iter().map(ToString::to_string).collect();
        let invocation = parse(&words).expect("parse");
        let harness = Harness::new("audit-no-tool");
        write_all_lock_families_with_advisories(&harness);
        harness.execute_with(&invocation, &runner)
    };
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("DX_GITLEAKS_BIN"), "{err}");
    assert!(runner.calls.borrow().is_empty(), "no ambient launch");
}

#[test]
pub(super) fn audit_live_vuln_findings_fail_and_git_is_incomplete() {
    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(&["security"], &runner, &|harness| {
        harness.write_source(
            "rust/tests/fixtures/hello/Cargo.lock",
            "[[package]]\nname = \"git-dep\"\nversion = \"0.1.0\"\nsource = \"git+https://github.com/example/git-dep#abc123\"\n",
        );
        write_npm_locks(harness);
        write_maven_lock(harness);
        write_nuget_lock(harness);
        write_go_mod(harness);
        write_ruby_locks(harness);
        write_powershell_locks(harness);
        write_all_empty_advisories(harness);
    });
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("incomplete"), "{err}");
}

#[test]
pub(super) fn audit_live_npm_git_and_sibling_locks_are_incomplete() {
    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(&["security"], &runner, &|harness| {
        write_cargo_lock(harness);
        write_npm_locks(harness);
        harness.write_source(
            "package-lock.json",
            r#"{"name":"root","lockfileVersion":3,"packages":{"":{"name":"root"},"node_modules/git-dep":{"version":"github:user/repo#abc123"}}}"#,
        );
        write_maven_lock(harness);
        write_nuget_lock(harness);
        write_go_mod(harness);
        write_ruby_locks(harness);
        write_powershell_locks(harness);
        write_all_empty_advisories(harness);
    });
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("incomplete"), "{err}");
    assert!(err.contains("git-dep"), "{err}");
}

#[test]
pub(super) fn audit_live_npm_pnpm_git_resolution_is_incomplete() {
    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(&["security"], &runner, &|harness| {
        write_cargo_lock(harness);
        harness.write_source(
            "pnpm-lock.yaml",
            "lockfileVersion: '9.0'\npackages:\n  'git-dep@github:user/repo#abc123':\n    resolution: {repo: 'https://github.com/user/repo.git', commit: abc123}\n",
        );
        write_maven_lock(harness);
        write_nuget_lock(harness);
        write_go_mod(harness);
        write_ruby_locks(harness);
        write_powershell_locks(harness);
        write_all_empty_advisories(harness);
    });
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("incomplete"), "{err}");
    assert!(err.contains("git-dep"), "{err}");
}

#[test]
pub(super) fn audit_live_vendored_mirror_analyzes_offline_like_upstream() {
    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(
        &["security", "//go/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_go_mod(harness);
            let json = r#"[{"id":"GHSA-go-test-0001","package":"github.com/google/go-cmp","versions":">=v0.5.0, <v0.7.0","severity":"high","fixed":["v0.7.0"],"set":"go"}]"#;
            let today = super::today_utc();
            let sha = dx_digest::sha256_hex(json.as_bytes());
            harness.write_source(".dx/advisory/go.json", json);
            let meta = serde_json::json!({
                "set": "go",
                "url": "file:///opt/dx-offline/advisory/go.json",
                "sha256": sha,
                "retrieved_at": today,
                "path": ".dx/advisory/go.json",
            });
            harness.write_source(".dx/advisory/go.meta.json", &meta.to_string());
        },
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("1 vulnerability findings"), "{err}");
}

#[test]
pub(super) fn audit_live_npm_family_locks_are_assessed_against_the_shared_npm_snapshot() {
    for (scope, rel) in [
        (
            "examples/adopt-js-ts/app",
            "examples/adopt-js-ts/pnpm-lock.yaml",
        ),
        (
            "examples/adopt-polyglot/frontend",
            "examples/adopt-polyglot/pnpm-lock.yaml",
        ),
        (
            "quality/tools/javascript/package.json",
            "quality/tools/javascript/pnpm-lock.yaml",
        ),
    ] {
        let runner = AuditRunner::clean();
        let (code, _out, err) = run_with(&["security", scope], &runner, &|harness| {
            harness.write_source(
                rel,
                "lockfileVersion: '9.0'\n\npackages:\n\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n",
            );
            write_advisory(
                harness,
                "npm",
                r#"[{"id":"GHSA-npm-test-0001","package":"react","versions":">=18.0.0, <18.3.0","severity":"high","fixed":["18.3.0"],"set":"npm"}]"#,
            );
        });
        assert_eq!(code, 1, "{scope} must fail: {err}");
        assert!(err.contains("audit_failed"), "{scope}: {err}");
        assert!(
            err.contains("1 vulnerability findings"),
            "{scope} must be assessed: {err}"
        );
    }
}

#[test]
pub(super) fn audit_live_npm_family_locks_are_clean_when_no_advisory_matches() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(&["security", "//..."], &runner, &|harness| {
        clean_workspace_with_advisories(harness);
    });
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit security: clean"), "{out}");
    assert!(
        !out.contains("no advisory coverage for npm-adopt"),
        "npm-adopt is covered: {out}"
    );
}

#[test]
pub(super) fn audit_live_ruby_scopes_are_assessed_against_the_rubygems_snapshot() {
    for scope in [
        "//third_party/ruby:default",
        "//examples/adopt-ruby/greet:greet",
        "//ruby/tests/fixtures/hello:hello",
    ] {
        let runner = AuditRunner::clean();
        let (code, _out, err) = run_with(&["security", scope], &runner, &|harness| {
            write_ruby_locks(harness);
            write_powershell_locks(harness);
            write_advisory(
                harness,
                "rubygems",
                r#"[{"id":"GHSA-ruby-test-0001","package":"rspec-core","versions":">=3.0.0, <3.13.1","severity":"high","fixed":["3.13.1"],"set":"ruby"}]"#,
            );
        });
        assert_eq!(code, 1, "{scope} must fail: {err}");
        assert!(
            err.contains("1 vulnerability findings"),
            "{scope} must be assessed: {err}"
        );
    }
}

#[test]
pub(super) fn audit_live_powershell_scopes_are_assessed_against_the_nuget_snapshot() {
    for scope in [
        "//third_party/powershell:default",
        "//examples/adopt-powershell/greet:greet",
    ] {
        let runner = AuditRunner::clean();
        let (code, _out, err) = run_with(&["security", scope], &runner, &|harness| {
            write_powershell_locks(harness);
            write_advisory(
                harness,
                "nuget",
                r#"[{"id":"GHSA-nuget-test-0001","package":"Pester","versions":"[5.0.0, 5.7.2)","severity":"high","fixed":["5.7.2"],"set":"nuget"}]"#,
            );
        });
        assert_eq!(code, 1, "{scope} must fail: {err}");
        assert!(
            err.contains("1 vulnerability findings"),
            "{scope} must be assessed: {err}"
        );
    }
}

#[test]
pub(super) fn audit_live_go_advisory_findings_fail_instead_of_empty_clean() {
    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(
        &["security", "//go/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_go_mod(harness);
            write_advisory(
                harness,
                "go",
                r#"[{"id":"GHSA-go-test-0001","package":"github.com/google/go-cmp","versions":">=v0.5.0, <v0.7.0","severity":"high","fixed":["v0.7.0"],"set":"go"}]"#,
            );
        },
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(err.contains("1 vulnerability findings"), "{err}");
}

const GO_ADVISORY: &str = r#"[{"id":"GHSA-go-test-0001","package":"github.com/google/go-cmp","versions":">=v0.5.0, <v0.7.0","severity":"high","fixed":["v0.7.0"],"set":"go"}]"#;
const GO_PACKAGE: &str = "github.com/google/go-cmp";

fn security_exception(set: &str, package: &str, versions: &str, expires: &str) -> String {
    let mut text = String::from("[[exception]]\nadvisory = \"GHSA-go-test-0001\"\n");
    for (key, value) in [
        ("package", package),
        ("set", set),
        ("versions", versions),
        ("reason", "reviewed"),
        ("expires", expires),
    ] {
        text.push_str(&format!("{key} = {value:?}\n"));
    }
    text
}

fn go_exception(versions: &str, expires: &str) -> String {
    security_exception("go", GO_PACKAGE, versions, expires)
}

fn security_with_go_advisory(security_toml: &str) -> (i32, String, String) {
    run_with(
        &["security", "//go/tests/fixtures/hello:hello"],
        &AuditRunner::clean(),
        &|harness| {
            write_go_mod(harness);
            write_advisory(harness, "go", GO_ADVISORY);
            harness.write_source("security.toml", security_toml);
        },
    )
}

#[test]
pub(super) fn audit_live_security_toml_exempts_only_a_matching_live_finding() {
    for (name, security_toml, expected, detail) in [
        (
            "cover",
            go_exception(">=v0.6.0, <v0.7.0", "2999-01-01"),
            0,
            "clean",
        ),
        (
            "wrong-version",
            go_exception(">=v0.7.0", "2999-01-01"),
            1,
            "1 vulnerability findings",
        ),
        (
            "wrong-package",
            security_exception("go", "other/module", ">=v0.6.0", "2999-01-01"),
            1,
            "on other/module matches no finding",
        ),
        (
            "expired",
            go_exception(">=v0.6.0, <v0.7.0", "2000-01-01"),
            1,
            "risk exception expired 2000-01-01",
        ),
    ] {
        let (code, out, err) = security_with_go_advisory(&security_toml);
        assert_eq!(code, expected, "{name}: {out}{err}");
        assert!(
            format!("{out}{err}").contains(detail),
            "{name} must report {detail:?}: {out}{err}"
        );
    }
}

#[test]
pub(super) fn audit_live_security_toml_exceptions_outside_the_scope_stay_unreported() {
    let security_toml = format!(
        "{}{}",
        go_exception(">=v0.6.0, <v0.7.0", "2999-01-01"),
        security_exception("npm", "react", ">=18.0.0", "2999-01-01")
    );
    let (code, out, err) = security_with_go_advisory(&security_toml);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit security: clean"), "{out}");
    assert!(
        !format!("{out}{err}").contains("matches no finding"),
        "{out}{err}"
    );
}

#[test]
pub(super) fn audit_live_invalid_security_toml_fails_without_assessing() {
    let (code, out, err) = security_with_go_advisory("schema_version = 99\n");
    assert_eq!(code, 1, "{out}{err}");
    let schema = "unsupported security.toml schema_version 99";
    assert!(err.contains(schema), "{err}");
    assert!(err.contains("security.toml"), "{err}");
}

#[test]
pub(super) fn audit_live_obsolete_security_toml_exception_fails_the_run() {
    let obsolete = security_exception("go", "github.com/spf13/cobra", ">=v1.0.0", "2999-01-01");
    let (code, out, err) = security_with_go_advisory(&obsolete);
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("matches no finding"), "{err}");
}

#[test]
pub(super) fn audit_live_missing_advisory_fails_never_empty_clean() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["security", "//rust/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_cargo_lock(harness);
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(
        err.contains(dx_audit::advisory::CODE_ADVISORY_REFRESH_FAILED),
        "{err}"
    );
    assert!(!out.contains("audit security: clean"), "{out}");
}

#[test]
pub(super) fn audit_live_stale_advisory_fails_without_stale_fallback() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["security", "//rust/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_cargo_lock(harness);
            let json = "[]";
            let url = dx_audit::advisory::advisory_source("cargo")
                .expect("source")
                .to_owned();
            let sha = dx_digest::sha256_hex(json.as_bytes());
            harness.write_source(".dx/advisory/cargo.json", json);
            let meta = serde_json::json!({
                "set": "cargo",
                "url": url,
                "sha256": sha,
                "retrieved_at": "2000-01-01",
                "path": ".dx/advisory/cargo.json",
            });
            harness.write_source(".dx/advisory/cargo.meta.json", &meta.to_string());
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(
        err.contains(dx_audit::advisory::CODE_ADVISORY_REFRESH_FAILED),
        "{err}"
    );
    assert!(err.contains("stale"), "{err}");
    assert!(!out.contains("audit security: clean"), "{out}");
}

#[test]
pub(super) fn audit_live_tampered_advisory_fails_on_sha_mismatch() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["security", "//rust/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_cargo_lock(harness);
            harness.write_source(".dx/advisory/cargo.json", "[]");
            let today = super::today_utc();
            let url = dx_audit::advisory::advisory_source("cargo")
                .expect("source")
                .to_owned();
            let meta = serde_json::json!({
                "set": "cargo",
                "url": url,
                "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "retrieved_at": today,
                "path": ".dx/advisory/cargo.json",
            });
            harness.write_source(".dx/advisory/cargo.meta.json", &meta.to_string());
        },
    );
    assert_eq!(code, 1, "{out}{err}");
    assert!(err.contains("audit_failed"), "{err}");
    assert!(
        err.contains(dx_audit::advisory::CODE_ADVISORY_REFRESH_FAILED),
        "{err}"
    );
    assert!(!out.contains("audit security: clean"), "{out}");
}

#[test]
pub(super) fn audit_live_license_clean_and_denied() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(&["license"], &runner, &|_harness| {});
    let _ = (code, out, err);
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(&["license"], &runner, &clean_workspace);
    assert_eq!(
        code, 1,
        "{out}{err} clean cargo license but missing notice plus UNKNOWN npm must fail distributed"
    );
    assert!(err.contains("audit_failed"), "{err}");

    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//rust/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_cargo_license_set(harness);
        },
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit license: clean"), "{out}");
}

#[test]
fn audit_live_license_names_uncovered_dependency_sets() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//go/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_go_mod(harness);
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"MIT\"]\nreview = []\ndeny = []\n\n[distribution]\ninternal = [\"//go/tests/fixtures/hello:hello\"]\n",
            );
        },
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit license: clean"), "{out}");
    assert!(!out.contains("no advisory coverage"), "{out}");

    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(&["license"], &runner, &clean_workspace);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("no advisory coverage for uv, uv-adopt, uv-adopt-polyglot, uv-tools"),
        "{err}"
    );
}

#[test]
pub(super) fn audit_live_license_per_ecosystem_ids_and_notice_texts() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//javascript/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            harness.write_source(
                "package-lock.json",
                r#"{"name":"root","lockfileVersion":3,"packages":{"":{"name":"root"},"node_modules/react":{"version":"18.2.0","license":"MIT"}}}"#,
            );
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"MIT\"]\nreview = []\ndeny = []\n\n[[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\nversions = \"18.2.0\"\ntext_present = true\n",
            );
        },
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit license: clean"), "{out}");

    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(
        &["license", "//javascript/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            harness.write_source(
                "package-lock.json",
                r#"{"name":"root","lockfileVersion":3,"packages":{"":{"name":"root"},"node_modules/react":{"version":"18.2.0","license":"MIT"}}}"#,
            );
            write_mit_licenses(harness);
        },
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");

    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//third_party/jvm:maven_install"],
        &runner,
        &|harness| {
            harness.write_source(
                "third_party/jvm/maven_install.json",
                r#"{"artifacts": {"junit:junit": {"version": "4.13.2"}}}"#,
            );
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"MIT\"]\nreview = [\"EPL-1.0\"]\ndeny = []\n\n[[exception]]\npackage = \"junit:junit\"\nset = \"maven\"\nlicense = \"EPL-1.0\"\nversions = \"4.13.2\"\nreason = \"Test approval.\"\nexpires = \"2027-03-01\"\n\n[[inventory]]\npackage = \"junit:junit\"\nset = \"maven\"\nlicense = \"EPL-1.0\"\nversions = \"4.13.2\"\ntext_present = true\n",
            );
        },
    );
    assert_eq!(code, 0, "{out}{err} {code}");
    assert!(out.contains("audit license: clean"), "{out}");

    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//csharp/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            harness.write_source(
                "third_party/dotnet/paket.lock",
                "NUGET\n  remote: https://api.nuget.org/v3/index.json\n    FSharp.Core (10.1.201)\n",
            );
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"MIT\"]\nreview = []\ndeny = []\n\n[[inventory]]\npackage = \"FSharp.Core\"\nset = \"nuget\"\nlicense = \"MIT\"\nversions = \"10.1.201\"\ntext_present = true\n",
            );
        },
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit license: clean"), "{out}");

    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//go/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            harness.write_source(
                "third_party/go/go.mod",
                "module example.com/root\n\ngo 1.24.12\n\nrequire example.com/hello v1.0.0\n",
            );
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"BSD-3-Clause\"]\nreview = []\ndeny = []\n\n[[inventory]]\npackage = \"example.com/hello\"\nset = \"go\"\nlicense = \"BSD-3-Clause\"\nversions = \"v1.0.0\"\ntext_present = true\n",
            );
        },
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit license: clean"), "{out}");

    let runner = AuditRunner::clean();
    let (code, _out, err) = run_with(
        &["license", "//go/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            harness.write_source(
                "third_party/go/go.mod",
                "module example.com/root\n\ngo 1.24.12\n\nrequire example.com/hello v1.0.0\n",
            );
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"BSD-3-Clause\"]\nreview = []\ndeny = []\n\n[[inventory]]\npackage = \"example.com/hello\"\nset = \"go\"\nlicense = \"BSD-3-Clause\"\nversions = \"v1.0.0\"\n",
            );
        },
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("audit_failed"), "{err}");
}

#[test]
pub(super) fn audit_live_target_scopes_to_owning_set_only() {
    let runner = AuditRunner::clean();
    let (code, out, err) = run_with(
        &["license", "//go/tests/fixtures/hello:hello"],
        &runner,
        &|harness| {
            write_go_mod(harness);
            harness.write_source(
                "licenses.toml",
                "[policy.distributed]\nallow = [\"MIT\"]\nreview = []\ndeny = []\n\n[distribution]\ninternal = [\"//go/tests/fixtures/hello:hello\"]\n",
            );
        },
    );
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("audit license: clean"), "{out}");
    assert_eq!(runner.calls.borrow().len(), 0);
}
