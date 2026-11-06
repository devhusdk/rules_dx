#[cfg(test)]
mod tests {
    use super::super::super::test_support::Harness;
    use super::super::super::test_support::{event_kinds, json_events};
    use super::super::audit_live::{write_advisory, AuditRunner};

    const UV_MODULE: &str = "uv = use_extension(\"@rules_py//uv:extensions.bzl\", \"uv\")\nuv.project(hub_name = \"pypi\", pyproject = \"//apps/frontend:pyproject.toml\", lock = \"//apps/frontend:uv.lock\")\nuv.project(hub_name = \"pypi\", pyproject = \"//services/worker:pyproject.toml\", lock = \"//services/worker:uv.lock\")\n";

    const PYPROJECT: &str = r#"
[project]
name = "project"
version = "0.1.0"
requires-python = ">=3.9"
dependencies = ["anyio>=4"]
"#;

    const UV_LOCK: &str = r#"
version = 1
requires-python = ">=3.9"

[[package]]
name = "anyio"
version = "4.0.0"
source = { registry = "https://pypi.org/simple" }
"#;

    fn consumer_harness(name: &str, module: &str, with_locks: bool) -> Harness {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let harness = Harness::new(&format!("audit-consumer-{name}-{id}"));
        harness.write_source("MODULE.bazel", module);
        harness.write_source("apps/frontend/pyproject.toml", PYPROJECT);
        harness.write_source("services/worker/pyproject.toml", PYPROJECT);
        if with_locks {
            harness.write_source("apps/frontend/uv.lock", UV_LOCK);
            harness.write_source("services/worker/uv.lock", UV_LOCK);
        }
        harness
    }

    fn run_with(argv: &[&str], runner: &AuditRunner, harness: &Harness) -> (i32, String, String) {
        let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        let invocation = dx_plan::args::parse(&words).expect("parse");
        harness.execute_with(&invocation, runner)
    }

    fn clean_runner() -> AuditRunner {
        AuditRunner::clean()
    }

    const NO_ADVISORIES: &str = "[]";

    const DJANGO_ADVISORY: &str = r#"[{
        "id": "PYSEC-2021-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "PyPI", "name": "Django"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "3.2"}, {"fixed": "3.2.5"}]}]
        }]
    }]"#;

    const SQLPARSE_ADVISORY: &str = r#"[{
        "id": "PYSEC-2021-0002",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "PyPI", "name": "SQLParse"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "0"}, {"fixed": "0.4.4"}]}]
        }]
    }]"#;

    fn lock_with(packages: &str) -> String {
        format!(
            "version = 1\nrequires-python = \">=3.9\"\n\n\
             [[package]]\nname = \"project\"\nversion = \"0.1.0\"\n\
             source = {{ virtual = \".\" }}\ndependencies = [{{ name = \"django\" }}]\n\n{packages}"
        )
    }

    fn registry_package(name: &str, version: &str) -> String {
        format!(
            "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n\
             source = {{ registry = \"https://pypi.org/simple\" }}\n\n"
        )
    }

    fn audited(
        name: &str,
        lock: &str,
        advisories: Option<&str>,
        extra: &[&str],
    ) -> (i32, String, String) {
        let harness = consumer_harness(name, UV_MODULE, true);
        harness.write_source("apps/frontend/uv.lock", lock);
        if let Some(json) = advisories {
            write_advisory(&harness, "pypi", json);
        }
        let runner = clean_runner();
        let mut argv = vec!["security", "--output=json", "pypi-frontend"];
        argv.extend_from_slice(extra);
        argv.extend_from_slice(&["--report", "sarif=audit.sarif"]);
        let (code, out, err) = run_with(&argv, &runner, &harness);
        let sarif =
            std::fs::read_to_string(harness.workspace.join("audit.sarif")).unwrap_or_default();
        (code, format!("{out}{sarif}"), err)
    }

    #[test]
    fn consumer_security_with_current_advisories_is_clean_with_no_coverage_gap() {
        let harness = consumer_harness("bare", UV_MODULE, true);
        write_advisory(&harness, "pypi", NO_ADVISORIES);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("audit_security_clean"), "{out}");
        assert!(!out.contains("no advisory coverage"), "{out}");
        for phantom in [
            "cargo",
            "pnpm",
            "maven",
            "nuget",
            "examples/",
            "third_party/",
        ] {
            assert!(!out.contains(phantom), "phantom {phantom} in {out}");
        }
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn consumer_detects_a_vulnerable_direct_package() {
        let lock = lock_with(&registry_package("django", "3.2.0"));
        let (code, out, err) = audited("direct", &lock, Some(DJANGO_ADVISORY), &[]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("1 vulnerability findings"), "{out}");
        assert!(out.contains("vuln/PYSEC-2021-0001"), "{out}");
        assert!(
            out.contains("django@3.2.0 vulnerable to PYSEC-2021-0001"),
            "{out}"
        );
        assert!(out.contains("fixed in 3.2.5"), "{out}");
        assert!(out.contains("apps/frontend/uv.lock"), "{out}");
    }

    #[test]
    fn consumer_detects_a_vulnerable_transitive_package() {
        let packages = format!(
            "{}{}",
            registry_package("django", "3.2.5"),
            registry_package("sqlparse", "0.4.1")
        );
        let (code, out, err) = audited(
            "transitive",
            &lock_with(&packages),
            Some(SQLPARSE_ADVISORY),
            &[],
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(
            out.contains("sqlparse@0.4.1 vulnerable to PYSEC-2021-0002"),
            "{out}"
        );
    }

    #[test]
    fn consumer_accepts_the_patched_version() {
        let packages = format!(
            "{}{}",
            registry_package("django", "3.2.5"),
            registry_package("sqlparse", "0.4.4")
        );
        let combined = DJANGO_ADVISORY.trim_end_matches(']').to_owned()
            + ","
            + SQLPARSE_ADVISORY.trim_start_matches('[');
        let (code, out, err) = audited("patched", &lock_with(&packages), Some(&combined), &[]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("audit_security_clean"), "{out}");
        assert!(!out.contains("vuln/"), "{out}");
    }

    #[test]
    fn consumer_stale_advisories_are_a_backend_failure_not_clean_or_a_finding() {
        let harness = consumer_harness("stale", UV_MODULE, true);
        let lock = lock_with(&registry_package("django", "3.2.0"));
        harness.write_source("apps/frontend/uv.lock", &lock);
        write_advisory(&harness, "pypi", DJANGO_ADVISORY);
        let meta_rel = ".dx/advisory/pypi.meta.json";
        let meta = std::fs::read_to_string(harness.workspace.join(meta_rel)).expect("meta");
        let mut value: serde_json::Value = serde_json::from_str(&meta).expect("json");
        value["retrieved_at"] = serde_json::json!("2020-01-01");
        harness.write_source(meta_rel, &value.to_string());
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "pypi-frontend"],
            &runner,
            &harness,
        );
        let all = format!("{out}{err}");
        assert_ne!(code, 0, "{all}");
        assert!(all.contains("failed to assess pypi-frontend"), "{all}");
        assert!(all.contains("advisory_refresh_failed"), "{all}");
        assert!(!all.contains("audit_security_clean"), "{all}");
        assert!(!all.contains("vulnerable to"), "{all}");
        let events = json_events(&out);
        let error = events
            .iter()
            .find(|event| event["event"] == "error")
            .expect("error event");
        assert_eq!(
            error["code"],
            serde_json::json!("advisory_refresh_failed"),
            "{out}"
        );
    }

    #[test]
    fn consumer_missing_advisories_fail_and_name_the_offline_requirement() {
        let lock = lock_with(&registry_package("django", "3.2.5"));
        let (code, out, err) = audited("missing", &lock, None, &["--offline"]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(out.contains("failed to assess pypi-frontend"), "{out}{err}");
        assert!(out.contains("offline_required"), "{out}{err}");
        assert!(out.contains(".dx/advisory/pypi.json"), "{out}{err}");
    }

    #[test]
    fn consumer_malformed_lock_is_incomplete() {
        let (code, out, err) = audited("malformed", "version = [", Some(NO_ADVISORIES), &[]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(out.contains("uv.lock"), "{out}{err}");
        assert!(!out.contains("audit_security_clean"), "{out}");
    }

    #[test]
    fn consumer_unassessed_sources_are_incomplete_not_clean() {
        for (case, source, reason) in [
            (
                "git",
                "{ git = \"https://example.com/x.git?rev=abc#abc\" }",
                "unsupported git revision",
            ),
            (
                "path",
                "{ path = \"vendor/x-1.0.tar.gz\" }",
                "unidentified private package",
            ),
            (
                "private",
                "{ registry = \"https://pypi.corp.example/simple\" }",
                "unidentified private package",
            ),
        ] {
            let packages = format!(
                "[[package]]\nname = \"x\"\nversion = \"1.0\"\nsource = {source}\n\n{}",
                registry_package("django", "3.2.5")
            );
            let (code, out, err) = audited(case, &lock_with(&packages), Some(NO_ADVISORIES), &[]);
            assert_ne!(code, 0, "{case}: {out}{err}");
            assert!(out.contains(reason), "{case}: {out}{err}");
            assert!(!out.contains("audit_security_clean"), "{case}: {out}");
        }
    }

    #[test]
    fn consumer_unparseable_locked_version_is_incomplete() {
        let lock = lock_with(&registry_package("django", "banana"));
        let (code, out, err) = audited("version", &lock, Some(DJANGO_ADVISORY), &[]);
        assert_ne!(code, 0, "{out}{err}");
        assert!(out.contains("unsupported version syntax"), "{out}{err}");
    }

    #[test]
    fn consumer_security_toml_exception_exempts_a_configured_set_finding() {
        let harness = consumer_harness("exception", UV_MODULE, true);
        harness.write_source(
            "apps/frontend/uv.lock",
            &lock_with(&registry_package("django", "3.2.0")),
        );
        write_advisory(&harness, "pypi", DJANGO_ADVISORY);
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"PYSEC-2021-0001\"\n\
             package = \"Django\"\nset = \"pypi-frontend\"\nversions = \">=3.2, <3.2.5\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "pypi-frontend"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/PYSEC"), "{out}");
    }

    #[test]
    fn consumer_name_and_scope_select_the_same_set() {
        let harness = consumer_harness("select", UV_MODULE, true);
        write_advisory(&harness, "pypi", NO_ADVISORIES);
        harness.write_source(
            "services/worker/uv.lock",
            &lock_with(&registry_package("django", "3.2.0")),
        );
        write_advisory(&harness, "pypi", DJANGO_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "pypi-frontend"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("PYSEC"), "{out}");
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &[
                "security",
                "--output=json",
                "services/worker/jobs",
                "--report",
                "sarif=select.sarif",
            ],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{out}{err}");
        let sarif = std::fs::read_to_string(harness.workspace.join("select.sarif")).expect("sarif");
        assert!(sarif.contains("services/worker/uv.lock"), "{sarif}");
        assert!(!sarif.contains("apps/frontend"), "{sarif}");
    }

    #[test]
    fn consumer_unknown_and_unowned_scopes_fail_closed() {
        let harness = consumer_harness("unknown", UV_MODULE, true);
        let runner = clean_runner();
        let (code, _, err) = run_with(&["security", "cargo"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("no owning dependency set"), "{err}");
        let runner = clean_runner();
        let (code, _, err) = run_with(&["security", "docs/cli/README.md"], &runner, &harness);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("no owning dependency set"), "{err}");
    }

    #[test]
    fn consumer_missing_lock_is_incomplete_in_both_families() {
        let harness = consumer_harness("missing-lock", UV_MODULE, false);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        assert!(err.contains("failed to assess pypi-frontend"), "{err}");
        assert!(err.contains("apps/frontend/uv.lock"), "{err}");
        let runner = clean_runner();
        let (code, _, err) = run_with(&["license", "pypi-worker"], &runner, &harness);
        assert_ne!(code, 0, "{err}");
        assert!(err.contains("failed to assess pypi-worker"), "{err}");
        assert!(err.contains("services/worker/uv.lock"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn consumer_symlinked_lock_leaving_the_workspace_fails_in_both_families() {
        let harness = consumer_harness("escaping-lock", UV_MODULE, false);
        let outside = harness.temp.join("uv.lock");
        std::fs::write(&outside, UV_LOCK).unwrap();
        for rel in ["apps/frontend/uv.lock", "services/worker/uv.lock"] {
            std::os::unix::fs::symlink(&outside, harness.workspace.join(rel)).unwrap();
        }
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "pypi-frontend"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to assess pypi-frontend"), "{err}");
        assert!(err.contains("apps/frontend/uv.lock"), "{err}");
        assert!(err.contains("leaves the workspace"), "{err}");
        let runner = clean_runner();
        let (code, out, err) = run_with(&["license", "pypi-worker"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to assess pypi-worker"), "{err}");
        assert!(err.contains("services/worker/uv.lock"), "{err}");
        assert!(err.contains("leaves the workspace"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn consumer_symlinked_lock_inside_the_workspace_is_read() {
        let harness = consumer_harness("inside-lock", UV_MODULE, false);
        harness.write_source("locks/uv.lock", UV_LOCK);
        for rel in ["apps/frontend/uv.lock", "services/worker/uv.lock"] {
            std::os::unix::fs::symlink(
                harness.workspace.join("locks/uv.lock"),
                harness.workspace.join(rel),
            )
            .unwrap();
        }
        let runner = clean_runner();
        let (code, out, err) = run_with(&["license", "--output=json"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn consumer_license_reports_no_packages_and_exits_zero() {
        let harness = consumer_harness("license", UV_MODULE, true);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["license", "--output=json"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let kinds = event_kinds(&events);
        assert!(kinds.contains(&"notice"), "{out}");
        assert_eq!(err, "", "{err}");
    }

    #[test]
    fn configured_and_builtin_selection_agree_on_names_and_scopes() {
        let registry = dx_adopt::dependency_sets::discover(UV_MODULE, &|rel| Err(rel.to_owned()))
            .expect("registry")
            .expect("present");
        for selector in [
            "pypi-frontend",
            "pypi-worker",
            "apps/frontend/app",
            "//...",
            "services/worker/...",
        ] {
            let update = dx_adopt::dependency_sets::resolve(&registry, &[selector.to_owned()])
                .expect("update resolves");
            let audit =
                super::super::resolve_configured_audit_sets(&registry, &[selector.to_owned()])
                    .expect("audit resolves");
            let mut update_names: Vec<&str> = update
                .iter()
                .map(|target| target.set.name.as_str())
                .collect();
            update_names.sort();
            let mut audit_names: Vec<&str> = audit.iter().map(|set| set.name.as_str()).collect();
            audit_names.sort();
            assert_eq!(update_names, audit_names, "{selector}");
        }
    }

    const NPM_MODULE: &str = "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(name = \"frontend\", pnpm_lock = \"//apps/web:pnpm-lock.yaml\")\n";

    const REACT_LOCK: &str =
        "lockfileVersion: '9.0'\n\npackages:\n\n  'react@18.2.0':\n    resolution: {integrity: sha512-abc}\n";

    const REACT_ADVISORY: &str = r#"[{
        "id": "GHSA-react-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "npm", "name": "react"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "18.0.0"}, {"fixed": "18.3.0"}]}]
        }]
    }]"#;

    fn npm_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, NPM_MODULE, false);
        harness.write_source("apps/web/package.json", "{}\n");
        harness.write_source("apps/web/pnpm-lock.yaml", REACT_LOCK);
        harness
    }

    #[test]
    fn consumer_npm_set_reads_its_lock_and_the_npm_snapshot() {
        let harness = npm_harness("npm-vuln");
        write_advisory(&harness, "npm", REACT_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "frontend"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/GHSA-react-0001"), "{out}");
        assert!(out.contains("apps/web/pnpm-lock.yaml"), "{out}");
        assert!(!out.contains("pypi"), "{out}");
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"GHSA-react-0001\"\n\
             package = \"react\"\nset = \"frontend\"\nversions = \">=18.0.0\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let (code, out, err) = run_with(
            &["security", "--output=json", "frontend"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/GHSA"), "{out}");
    }

    #[test]
    fn consumer_npm_missing_snapshot_fails_and_is_never_clean() {
        let harness = npm_harness("npm-missing");
        write_advisory(&harness, "pypi", NO_ADVISORIES);
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "--offline", "frontend"],
            &runner,
            &harness,
        );
        assert_ne!(code, 0, "{out}{err}");
        assert!(out.contains("failed to assess frontend"), "{out}{err}");
        assert!(out.contains(".dx/advisory/npm.json"), "{out}{err}");
        assert!(!out.contains("no advisory coverage"), "{out}");
        assert!(!out.contains("audit_security_clean"), "{out}");
    }

    #[test]
    fn consumer_npm_license_reports_unknown_until_inventoried() {
        let harness = npm_harness("npm-license");
        let runner = clean_runner();
        let (code, out, err) =
            run_with(&["license", "--output=json", "frontend"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("react"), "{code} {out}{err}");
        assert!(out.contains("UNKNOWN"), "{out}");
        harness.write_source(
            "licenses.toml",
            "[policy.distributed]\nallow = [\"MIT\", \"Apache-2.0\"]\nreview = []\ndeny = []\n\n\
             [[inventory]]\npackage = \"react\"\nset = \"npm\"\nlicense = \"MIT\"\n\
             versions = \"18.2.0\"\ntext_present = true\n",
        );
        let (code, out, err) =
            run_with(&["license", "--output=json", "frontend"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("UNKNOWN"), "{out}");
    }

    const PINNED_MODULE: &str = "ruby = use_extension(\"@rules_dx//ruby:extensions.bzl\", \"ruby\")\nruby.bundle_fetch(name = \"gems\", gemfile = \"//apps/rb:Gemfile\", gemfile_lock = \"//apps/rb:Gemfile.lock\")\n";

    const GEMFILE_LOCK: &str = "GEM\n  remote: https://rubygems.org/\n  specs:\n    rack (2.2.0)\n\nPLATFORMS\n  ruby\n\nDEPENDENCIES\n  rack\n";

    const RACK_ADVISORY: &str = r#"[{
        "id": "GHSA-rack-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "RubyGems", "name": "rack"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "2.0.0"}, {"fixed": "2.2.8"}]}]
        }]
    }]"#;

    fn pinned_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, PINNED_MODULE, false);
        harness.write_source(
            "apps/rb/Gemfile",
            "source \"https://rubygems.org\"\ngem \"rack\"\n",
        );
        harness.write_source("apps/rb/Gemfile.lock", GEMFILE_LOCK);
        harness
    }

    #[test]
    fn consumer_ruby_set_reads_its_gemfile_lock_and_the_rubygems_snapshot() {
        let harness = pinned_harness("ruby-vuln");
        write_advisory(&harness, "rubygems", RACK_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json", "gems"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/GHSA-rack-0001"), "{out}");
        assert!(out.contains("apps/rb/Gemfile.lock"), "{out}");
        assert!(!out.contains("pypi"), "{out}");
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"GHSA-rack-0001\"\n\
             package = \"rack\"\nset = \"gems\"\nversions = \">= 2.0.0\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let (code, out, err) = run_with(&["security", "--output=json", "gems"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/GHSA"), "{out}");
    }

    const COMPOSER_MODULE: &str = "composer = use_extension(\"@rules_dx//php/composer:extensions.bzl\", \"composer\")\ncomposer.lock(name = \"vendor\", lock = \"//apps/php:composer.lock\", digests = \"//apps/php:composer.digests.json\")\n";

    const COMPOSER_LOCK: &str = r#"{"packages": [{"name": "psr/http-message", "version": "v2.0", "dist": {"reference": "402d35bc"}}], "packages-dev": []}"#;

    const COMPOSER_ADVISORY: &str = r#"[{
        "id": "GHSA-psr-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "Packagist", "name": "psr/http-message"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "1.0.0"}, {"fixed": "2.1.0"}]}]
        }]
    }]"#;

    #[test]
    fn consumer_composer_set_reads_its_composer_lock_and_the_packagist_snapshot() {
        let harness = consumer_harness("composer-vuln", COMPOSER_MODULE, false);
        harness.write_source("apps/php/composer.lock", COMPOSER_LOCK);
        harness.write_source(
            "apps/php/composer.digests.json",
            r#"{"packages": {"psr/http-message": {"reference": "402d35bc", "sha256": "00"}}}"#,
        );
        write_advisory(&harness, "packagist", COMPOSER_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) =
            run_with(&["security", "--output=json", "vendor"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/GHSA-psr-0001"), "{out}");
        assert!(out.contains("apps/php/composer.lock"), "{out}");
    }

    #[test]
    fn consumer_ruby_missing_snapshot_fails_and_is_never_clean() {
        for extra in [None, Some("--offline")] {
            let harness = pinned_harness("ruby-missing");
            write_advisory(&harness, "pypi", NO_ADVISORIES);
            let runner = clean_runner();
            let mut argv = vec!["security", "--output=json"];
            argv.extend(extra);
            argv.push("gems");
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{extra:?} {out}{err}");
            assert!(out.contains("failed to assess gems"), "{out}{err}");
            assert!(out.contains(".dx/advisory/rubygems.json"), "{out}{err}");
            assert!(!out.contains("no advisory coverage"), "{out}");
            assert!(!out.contains("audit_security_clean"), "{out}");
            if extra.is_some() {
                assert!(out.contains("offline_required"), "{out}{err}");
            }
        }
    }

    #[test]
    fn consumer_pinned_license_uses_the_ecosystem_inventory_keys() {
        let harness = pinned_harness("pinned-license");
        let runner = clean_runner();
        let (_, out, err) = run_with(&["license", "--output=json"], &runner, &harness);
        assert!(out.contains("rack"), "{out}{err}");
        assert!(out.contains("UNKNOWN"), "{out}");
        assert!(!out.contains("failed to assess"), "{out}");
        harness.write_source(
            "licenses.toml",
            "[policy.distributed]\nallow = [\"MIT\", \"Apache-2.0\"]\nreview = []\ndeny = []\n\n\
             [[inventory]]\npackage = \"rack\"\nset = \"ruby\"\nlicense = \"MIT\"\n\
             versions = \"2.2.0\"\ntext_present = true\n",
        );
        let (code, out, err) = run_with(&["license", "--output=json"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("UNKNOWN"), "{out}");
    }

    const CARGO_MODULE: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"crates\", cargo_lockfile = \"//apps/rs:Cargo.lock\", lockfile = \"//apps/rs:cargo-bazel-lock.json\", manifests = [\"//apps/rs:Cargo.toml\"])\nuse_repo(crate, \"crates\")\n";

    const SMALLVEC_LOCK: &str = "version = 4\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"smallvec\"]\n\n[[package]]\nname = \"smallvec\"\nversion = \"1.6.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"fe0f37c9e8f3c5a4a66ad655a93c74daac4ad00c441533bf5c6e7990bb42604e\"\n";

    const SMALLVEC_ADVISORY: &str = r#"[{
        "id": "RUSTSEC-2021-0003",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "crates.io", "name": "smallvec"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "1.6.0"}, {"fixed": "1.6.1"}]}]
        }]
    }]"#;

    fn cargo_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, CARGO_MODULE, false);
        harness.write_source(
            "apps/rs/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n",
        );
        harness.write_source("apps/rs/Cargo.lock", SMALLVEC_LOCK);
        harness.write_source(
            "apps/rs/cargo-bazel-lock.json",
            "{\"crates\": {\"smallvec 1.6.0\": {\"name\": \"smallvec\", \"version\": \"1.6.0\", \"license\": \"MIT OR Apache-2.0\"}}}\n",
        );
        harness
    }

    #[test]
    fn consumer_cargo_set_reads_only_its_cargo_lock_and_the_cargo_snapshot() {
        let harness = cargo_harness("cargo-vuln");
        harness.write_source("Cargo.lock", "not a lock\n");
        write_advisory(&harness, "cargo", SMALLVEC_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) =
            run_with(&["security", "--output=json", "crates"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/RUSTSEC-2021-0003"), "{out}");
        assert!(out.contains("apps/rs/Cargo.lock"), "{out}");
        assert!(!out.contains("pypi"), "{out}");
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"RUSTSEC-2021-0003\"\n\
             package = \"smallvec\"\nset = \"crates\"\nversions = \">=1.6.0\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let (code, out, err) =
            run_with(&["security", "--output=json", "crates"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/RUSTSEC"), "{out}");
    }

    #[test]
    fn consumer_cargo_missing_snapshot_fails_and_is_never_clean() {
        for extra in [None, Some("--offline")] {
            let harness = cargo_harness("cargo-missing");
            write_advisory(&harness, "pypi", NO_ADVISORIES);
            let runner = clean_runner();
            let mut argv = vec!["security", "--output=json"];
            argv.extend(extra);
            argv.push("crates");
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{extra:?} {out}{err}");
            assert!(out.contains("failed to assess crates"), "{out}{err}");
            assert!(out.contains(".dx/advisory/cargo.json"), "{out}{err}");
            assert!(!out.contains("no advisory coverage"), "{out}");
            assert!(!out.contains("audit_security_clean"), "{out}");
            if extra.is_some() {
                assert!(out.contains("offline_required"), "{out}{err}");
            }
        }
    }

    #[test]
    fn consumer_cargo_license_comes_from_the_declared_cargo_bazel_lock() {
        let harness = cargo_harness("cargo-license");
        let runner = clean_runner();
        let (_, out, err) = run_with(&["license", "--output=json", "crates"], &runner, &harness);
        assert!(out.contains("smallvec"), "{out}{err}");
        assert!(out.contains("MIT OR Apache-2.0"), "{out}{err}");
        assert!(!out.contains("UNKNOWN"), "{out}");
        assert!(!out.contains("failed to assess"), "{out}");
    }
    const PUB_MODULE: &str = "pub = use_extension(\"@rules_dart//dart/pub:extensions.bzl\", \"pub\")\npub.from_lock(name = \"dart_deps\", lock = \"//apps/dart:pubspec.lock\")\n";

    const PUB_ADVISORY: &str = r#"[{
        "id": "GHSA-pub-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "Pub", "name": "http"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "0.13.3"}]}]
        }]
    }]"#;

    fn pub_harness(name: &str, version: &str) -> Harness {
        let harness = consumer_harness(name, PUB_MODULE, false);
        harness.write_source(
            "apps/dart/pubspec.yaml",
            "name: app\ndependencies:\n  http: ^0.13.0\n",
        );
        harness.write_source(
            "apps/dart/pubspec.lock",
            &format!(
                "packages:\n  http:\n    dependency: \"direct main\"\n    description:\n      name: http\n      url: \"https://pub.dev\"\n    source: hosted\n    version: \"{version}\"\n"
            ),
        );
        harness
    }

    #[test]
    fn consumer_pub_set_reports_a_vulnerable_lock_and_passes_the_fixed_one() {
        let harness = pub_harness("pub-vuln", "0.13.2");
        write_advisory(&harness, "pub", PUB_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "dart_deps"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/GHSA-pub-0001"), "{out}");
        assert!(out.contains("apps/dart/pubspec.lock"), "{out}");
        let fixed = pub_harness("pub-fixed", "0.13.3");
        write_advisory(&fixed, "pub", PUB_ADVISORY);
        let (code, out, err) =
            run_with(&["security", "--output=json", "dart_deps"], &runner, &fixed);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/GHSA"), "{out}");
    }

    const SWIFT_MODULE: &str = "swift_deps = use_extension(\"@rules_swift_package_manager//:extensions.bzl\", \"swift_deps\")\nswift_deps.from_package(resolved = \"//apps/swift:Package.resolved\", swift = \"//apps/swift:Package.swift\")\n";

    const SWIFT_ADVISORY: &str = r#"[{
        "id": "GHSA-swift-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "SwiftURL", "name": "github.com/apple/swift-argument-parser"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "1.8.3"}]}]
        }]
    }]"#;

    fn swift_harness(name: &str, version: &str) -> Harness {
        let harness = consumer_harness(name, SWIFT_MODULE, false);
        harness.write_source(
            "apps/swift/Package.swift",
            "// swift-tools-version: 6.0\nimport PackageDescription\nlet package = Package(name: \"app\", dependencies: [.package(url: \"https://github.com/apple/swift-argument-parser\", exact: \"1.8.2\")], targets: [])\n",
        );
        harness.write_source(
            "apps/swift/Package.resolved",
            &format!(
                "{{\"pins\":[{{\"identity\":\"swift-argument-parser\",\"kind\":\"remoteSourceControl\",\"location\":\"https://github.com/apple/swift-argument-parser\",\"state\":{{\"revision\":\"abc\",\"version\":\"{version}\"}}}}],\"version\":3}}"
            ),
        );
        harness
    }

    const HEX_MODULE: &str = "elixir = use_extension(\"//elixir/toolchain:repos.bzl\", \"elixir\")\nelixir.mix(name = \"deps\", lock = \"//third_party/elixir:mix.lock\")\n";

    const HEX_ADVISORY: &str = r#"[{
        "id": "GHSA-hex-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "Hex", "name": "plug"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "1.15.1"}]}]
        }]
    }]"#;

    fn hex_harness(name: &str, version: &str) -> Harness {
        let harness = consumer_harness(name, HEX_MODULE, false);
        harness.write_source("third_party/elixir/mix.exs", "defmodule App do\nend\n");
        harness.write_source(
            "third_party/elixir/mix.lock",
            &format!(
                "%{{\n  \"plug\": {{:hex, :plug, \"{version}\", \"h\", [:mix], [], \"hexpm\", \"h2\"}},\n}}\n"
            ),
        );
        harness
    }

    #[test]
    fn consumer_swiftpm_set_reports_a_vulnerable_pin_and_passes_the_fixed_one() {
        let harness = swift_harness("swift-vuln", "1.8.2");
        write_advisory(&harness, "swifturl", SWIFT_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) =
            run_with(&["security", "--output=json", "swiftpm"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/GHSA-swift-0001"), "{out}");
        assert!(out.contains("apps/swift/Package.resolved"), "{out}");
        let fixed = swift_harness("swift-fixed", "1.8.3");
        write_advisory(&fixed, "swifturl", SWIFT_ADVISORY);
        let (code, out, err) = run_with(&["security", "--output=json", "swiftpm"], &runner, &fixed);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/GHSA"), "{out}");
    }

    #[test]
    fn consumer_hex_set_reports_a_vulnerable_lock_and_passes_the_fixed_one() {
        let harness = hex_harness("hex-vuln", "1.15.0");
        write_advisory(&harness, "hex", HEX_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json", "deps"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/GHSA-hex-0001"), "{out}");
        assert!(out.contains("third_party/elixir/mix.lock"), "{out}");
        let fixed = hex_harness("hex-fixed", "1.15.1");
        write_advisory(&fixed, "hex", HEX_ADVISORY);
        let (code, out, err) = run_with(&["security", "--output=json", "deps"], &runner, &fixed);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("GHSA-hex"), "{out}");
    }

    const MAVEN_MODULE: &str = "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\nmaven.install(lock_file = \"//third_party:maven_install.json\")\nuse_repo(maven, \"maven\")\n";

    const MAVEN_ADVISORY: &str = r#"[{
        "id": "GHSA-maven-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "Maven", "name": "com.example:lib"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "1.0"}, {"fixed": "1.2"}]}]
        }]
    }]"#;

    fn maven_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, MAVEN_MODULE, false);
        harness.write_source(
            "third_party/maven_install.json",
            "{\"__INPUT_ARTIFACTS_HASH\":1,\"__RESOLVED_ARTIFACTS_HASH\":2,\"artifacts\":{\"com.example:lib\":{\"shasums\":{\"jar\":\"x\"},\"version\":\"1.1\"}}}\n",
        );
        harness
    }

    #[test]
    fn consumer_maven_set_reads_its_declared_lock_and_the_maven_snapshot() {
        let harness = maven_harness("maven-vuln");
        write_advisory(&harness, "maven", MAVEN_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json", "maven"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("GHSA-maven-0001"), "{out}");
        assert!(out.contains("third_party/maven_install.json"), "{out}");
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"GHSA-maven-0001\"\n\
             package = \"com.example:lib\"\nset = \"maven\"\nversions = \"1.1\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let (code, out, err) = run_with(&["security", "--output=json", "maven"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/GHSA"), "{out}");
    }

    #[test]
    fn consumer_maven_missing_snapshot_fails_and_is_never_clean() {
        for extra in [None, Some("--offline")] {
            let harness = maven_harness("maven-missing");
            write_advisory(&harness, "pypi", NO_ADVISORIES);
            let runner = clean_runner();
            let mut argv = vec!["security", "--output=json"];
            argv.extend(extra);
            argv.push("maven");
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{extra:?} {out}{err}");
            assert!(out.contains("failed to assess maven"), "{out}{err}");
            assert!(out.contains(".dx/advisory/maven.json"), "{out}{err}");
            assert!(!out.contains("audit_security_clean"), "{out}");
            if extra.is_some() {
                assert!(out.contains("offline_required"), "{out}{err}");
            }
        }
    }

    const NUGET_MODULE: &str =
        "paket = use_extension(\"//dn/deps:paket.main_extension.bzl\", \"main_extension\")\n";

    const NUGET_ADVISORY: &str = r#"[{
        "id": "GHSA-nuget-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "NuGet", "name": "Example.Lib"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "1.0"}, {"fixed": "1.2"}]}]
        }]
    }]"#;

    fn nuget_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, NUGET_MODULE, false);
        harness.write_source(
            "dn/paket.dependencies",
            "source https://api.nuget.org/v3/index.json\nnuget Example.Lib\n",
        );
        harness.write_source(
            "dn/paket.lock",
            "NUGET\n  remote: https://api.nuget.org/v3/index.json\n    Example.Lib (1.1)\n",
        );
        harness.write_source("dn/deps/paket.main.bzl", "main\n");
        harness
    }

    #[test]
    fn consumer_nuget_set_reads_its_paket_lock_and_the_nuget_snapshot() {
        let harness = nuget_harness("nuget-vuln");
        write_advisory(&harness, "nuget", NUGET_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json", "nuget"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("GHSA-nuget-0001"), "{out}");
        assert!(out.contains("dn/paket.lock"), "{out}");
    }

    #[test]
    fn consumer_nuget_missing_snapshot_fails_and_is_never_clean() {
        let harness = nuget_harness("nuget-missing");
        write_advisory(&harness, "pypi", NO_ADVISORIES);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json", "nuget"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        assert!(out.contains("failed to assess nuget"), "{out}{err}");
        assert!(out.contains(".dx/advisory/nuget.json"), "{out}{err}");
        assert!(!out.contains("audit_security_clean"), "{out}");
    }

    const GO_MODULE: &str = "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\ngo_deps.from_file(go_mod = \"//g:go.mod\")\n";

    const GO_ADVISORY: &str = r#"[{
        "id": "GHSA-go-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "Go", "name": "github.com/example/lib"},
            "ranges": [{"type": "SEMVER", "events": [{"introduced": "0"}, {"fixed": "1.2.0"}]}]
        }]
    }]"#;

    fn go_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, GO_MODULE, false);
        harness.write_source(
            "g/go.mod",
            "module example.com/m\n\ngo 1.22\n\nrequire github.com/example/lib v1.1.0\n",
        );
        harness.write_source("g/go.sum", "github.com/example/lib v1.1.0 h1:abc=\n");
        harness
    }

    #[test]
    fn consumer_go_set_reads_its_go_mod_and_the_go_snapshot() {
        let harness = go_harness("go-vuln");
        write_advisory(&harness, "go", GO_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json", "go"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("GHSA-go-0001"), "{out}");
        assert!(out.contains("g/go.mod"), "{out}");
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"GHSA-go-0001\"\n\
             package = \"github.com/example/lib\"\nset = \"go\"\nversions = \"1.1.0\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let (code, out, err) = run_with(&["security", "--output=json", "go"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/GHSA"), "{out}");
    }

    #[test]
    fn consumer_go_missing_snapshot_fails_and_is_never_clean() {
        for extra in [None, Some("--offline")] {
            let harness = go_harness("go-missing");
            write_advisory(&harness, "pypi", NO_ADVISORIES);
            let runner = clean_runner();
            let mut argv = vec!["security", "--output=json"];
            argv.extend(extra);
            argv.push("go");
            let (code, out, err) = run_with(&argv, &runner, &harness);
            assert_ne!(code, 0, "{extra:?} {out}{err}");
            assert!(out.contains("failed to assess go"), "{out}{err}");
            assert!(out.contains(".dx/advisory/go.json"), "{out}{err}");
            assert!(!out.contains("audit_security_clean"), "{out}");
            if extra.is_some() {
                assert!(out.contains("offline_required"), "{out}{err}");
            }
        }
    }

    const CPAN_MODULE: &str = "cpan = use_extension(\"@rules_perl//perl/cpan:extensions.bzl\", \"cpan\")\ncpan.install(name = \"perl_app_cpan\", lock = \"//perl/third_party:cpanfile.snapshot.lock.json\")\n";

    const CPANSA: &str = "{\"Mojolicious\":{\"distribution\":\"Mojolicious\",\"advisories\":[{\"id\":\"CPANSA-Mojolicious-2018-03\",\"affected_versions\":[\">=7.0,<7.58\"],\"fixed_versions\":[\">=7.58\"],\"severity\":\"high\"}]}}";

    fn cpan_harness(name: &str, release: &str) -> Harness {
        let harness = consumer_harness(name, CPAN_MODULE, false);
        harness.write_source("perl/third_party/cpanfile", "requires 'Mojolicious';\n");
        harness.write_source(
            "perl/third_party/cpanfile.snapshot",
            &format!(
                "# carton snapshot format: version 1.0\nDISTRIBUTIONS\n  {release}\n    pathname: S/SR/SRI/{release}.tar.gz\n    provides:\n      Mojolicious {}\n",
                release.trim_start_matches("Mojolicious-")
            ),
        );
        harness.write_source(
            "perl/third_party/cpanfile.snapshot.lock.json",
            &format!("{{\"Mojolicious\":{{\"strip_prefix\":\"{release}\"}}}}\n"),
        );
        let prepared = dx_audit::advisory_prep::cpan_snapshot(
            CPANSA,
            "1.002",
            super::super::super::test_support::TEST_AUDIT_TODAY,
        )
        .expect("prepared");
        harness.write_source(
            &prepared.snapshot.path,
            std::str::from_utf8(&prepared.payload).expect("utf8"),
        );
        harness.write_source(
            &dx_audit::advisory::identity_rel("cpan"),
            &serde_json::to_string(&prepared.snapshot).expect("identity"),
        );
        harness
    }

    #[test]
    fn consumer_cpan_set_reads_its_snapshot_and_the_cpansa_advisories() {
        let harness = cpan_harness("cpan-vuln", "Mojolicious-7.57");
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "perl_app_cpan"],
            &runner,
            &harness,
        );
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/CPANSA-Mojolicious-2018-03"), "{out}");
        assert!(out.contains("perl/third_party/cpanfile.snapshot"), "{out}");
        assert!(!out.contains("no advisory coverage"), "{out}");
    }

    #[test]
    fn consumer_cpan_set_with_the_patched_release_is_clean() {
        let harness = cpan_harness("cpan-clean", "Mojolicious-7.58");
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "perl_app_cpan"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/CPANSA"), "{out}");
    }

    const CABAL_MODULE: &str = "haskell = use_extension(\"//haskell/toolchain:repos.bzl\", \"haskell\")\nhaskell.packages(name = \"hackage\", deps = \"//hs:haskell_deps.json\")\n";

    const HACKAGE_ADVISORY: &str = r#"[{
        "id": "HSEC-2026-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "Hackage", "name": "safe"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "0"}, {"fixed": "0.3.22"}]}]
        }]
    }]"#;

    #[test]
    fn consumer_cabal_set_reads_its_freeze_and_the_hackage_snapshot() {
        let harness = consumer_harness("cabal-vuln", CABAL_MODULE, false);
        harness.write_source("hs/cabal.project", "packages: .\n");
        harness.write_source(
            "hs/cabal.project.freeze",
            "constraints: any.base ==4.20.2.0,\n             any.safe ==0.3.21\nindex-state: hackage.haskell.org 2026-09-30T23:43:34Z\n",
        );
        harness.write_source(
            "hs/haskell_deps.json",
            "{\"packages\":[{\"name\":\"base\",\"version\":\"4.20.2.0\"},{\"name\":\"safe\",\"version\":\"0.3.21\"}]}\n",
        );
        write_advisory(&harness, "hackage", HACKAGE_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) =
            run_with(&["security", "--output=json", "hackage"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("vuln/HSEC-2026-0001"), "{out}");
        assert!(out.contains("hs/cabal.project.freeze"), "{out}");
    }
}
