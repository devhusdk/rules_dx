#[cfg(test)]
mod tests {
    use super::super::super::test_support::Harness;
    use super::super::super::test_support::{event_kinds, json_events};
    use super::super::audit_live::{write_advisory, AuditRunner};

    const DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]

[[dependency_set]]
name = "worker"
ecosystem = "uv"
manifests = ["services/worker/pyproject.toml"]
locks = ["services/worker/uv.lock"]
scopes = ["services/worker"]
"#;

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

    fn consumer_harness(name: &str, dx_toml: &str, with_locks: bool) -> Harness {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let harness = Harness::new(&format!("audit-consumer-{name}-{id}"));
        harness.write_source("dx.toml", dx_toml);
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
        let invocation = crate::args::parse(&words).expect("parse");
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
        let harness = consumer_harness(name, DX_TOML, true);
        harness.write_source("apps/frontend/uv.lock", lock);
        if let Some(json) = advisories {
            write_advisory(&harness, "pypi", json);
        }
        let runner = clean_runner();
        let mut argv = vec!["security", "--output=json", "frontend"];
        argv.extend_from_slice(extra);
        argv.extend_from_slice(&["--report", "sarif=audit.sarif"]);
        let (code, out, err) = run_with(&argv, &runner, &harness);
        let sarif =
            std::fs::read_to_string(harness.workspace.join("audit.sarif")).unwrap_or_default();
        (code, format!("{out}{sarif}"), err)
    }

    #[test]
    fn consumer_security_with_current_advisories_is_clean_with_no_coverage_gap() {
        let harness = consumer_harness("bare", DX_TOML, true);
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
        let combined = format!(
            "{}",
            DJANGO_ADVISORY.trim_end_matches(']').to_owned()
                + ","
                + SQLPARSE_ADVISORY.trim_start_matches('[')
        );
        let (code, out, err) = audited("patched", &lock_with(&packages), Some(&combined), &[]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("audit_security_clean"), "{out}");
        assert!(!out.contains("vuln/"), "{out}");
    }

    #[test]
    fn consumer_stale_advisories_are_a_backend_failure_not_clean_or_a_finding() {
        let harness = consumer_harness("stale", DX_TOML, true);
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
            &["security", "--output=json", "frontend"],
            &runner,
            &harness,
        );
        let all = format!("{out}{err}");
        assert_ne!(code, 0, "{all}");
        assert!(all.contains("failed to assess frontend"), "{all}");
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
        assert!(out.contains("failed to assess frontend"), "{out}{err}");
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
        let harness = consumer_harness("exception", DX_TOML, true);
        harness.write_source(
            "apps/frontend/uv.lock",
            &lock_with(&registry_package("django", "3.2.0")),
        );
        write_advisory(&harness, "pypi", DJANGO_ADVISORY);
        harness.write_source(
            "security.toml",
            "schema_version = 1\n[[exception]]\nadvisory = \"PYSEC-2021-0001\"\n\
             package = \"Django\"\nset = \"frontend\"\nversions = \">=3.2, <3.2.5\"\n\
             reason = \"reviewed\"\nexpires = \"2999-01-01\"\n",
        );
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "frontend"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("vuln/PYSEC"), "{out}");
    }

    #[test]
    fn consumer_name_and_scope_select_the_same_set() {
        let harness = consumer_harness("select", DX_TOML, true);
        write_advisory(&harness, "pypi", NO_ADVISORIES);
        harness.write_source(
            "services/worker/uv.lock",
            &lock_with(&registry_package("django", "3.2.0")),
        );
        write_advisory(&harness, "pypi", DJANGO_ADVISORY);
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "frontend"],
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
        let harness = consumer_harness("unknown", DX_TOML, true);
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
        let harness = consumer_harness("missing-lock", DX_TOML, false);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        assert!(err.contains("failed to assess frontend"), "{err}");
        assert!(err.contains("apps/frontend/uv.lock"), "{err}");
        let runner = clean_runner();
        let (code, _, err) = run_with(&["license", "worker"], &runner, &harness);
        assert_ne!(code, 0, "{err}");
        assert!(err.contains("failed to assess worker"), "{err}");
        assert!(err.contains("services/worker/uv.lock"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn consumer_symlinked_lock_leaving_the_workspace_fails_in_both_families() {
        let harness = consumer_harness("escaping-lock", DX_TOML, false);
        let outside = harness.temp.join("uv.lock");
        std::fs::write(&outside, UV_LOCK).unwrap();
        for rel in ["apps/frontend/uv.lock", "services/worker/uv.lock"] {
            std::os::unix::fs::symlink(&outside, harness.workspace.join(rel)).unwrap();
        }
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "frontend"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to assess frontend"), "{err}");
        assert!(err.contains("apps/frontend/uv.lock"), "{err}");
        assert!(err.contains("leaves the workspace"), "{err}");
        let runner = clean_runner();
        let (code, out, err) = run_with(&["license", "worker"], &runner, &harness);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("failed to assess worker"), "{err}");
        assert!(err.contains("services/worker/uv.lock"), "{err}");
        assert!(err.contains("leaves the workspace"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn consumer_symlinked_lock_inside_the_workspace_is_read() {
        let harness = consumer_harness("inside-lock", DX_TOML, false);
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
        let harness = consumer_harness("license", DX_TOML, true);
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
        let registry = dx_adopt::dependency_sets::parse(DX_TOML, "dx.toml")
            .expect("registry")
            .expect("present");
        for selector in [
            "frontend",
            "worker",
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

    const NPM_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "npm"
manifests = ["apps/web/package.json"]
locks = ["apps/web/pnpm-lock.yaml"]
scopes = ["apps/web"]
"#;

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
        let harness = consumer_harness(name, NPM_TOML, false);
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
        assert!(out.contains("react"), "{code} {out}{err}");
        assert!(out.contains("UNKNOWN"), "{out}");
    }

    const PINNED_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "gems"
ecosystem = "ruby"
manifests = ["apps/rb/Gemfile"]
locks = ["apps/rb/Gemfile.lock"]
scopes = ["apps/rb"]

[[dependency_set]]
name = "modules"
ecosystem = "powershell"
manifests = ["tools/ps/PSGallery.requirements.psd1"]
locks = ["tools/ps/PSGallery.lock.json"]
scopes = ["tools/ps"]
"#;

    const GEMFILE_LOCK: &str = "GEM\n  remote: https://rubygems.org/\n  specs:\n    rack (2.2.0)\n\nPLATFORMS\n  ruby\n\nDEPENDENCIES\n  rack\n";

    const PSGALLERY_LOCK: &str =
        "{\n  \"modules\": {\n    \"Pester\": {\n      \"version\": \"5.7.1\"\n    }\n  }\n}\n";

    const RACK_ADVISORY: &str = r#"[{
        "id": "GHSA-rack-0001",
        "modified": "2026-09-18T00:00:00Z",
        "affected": [{
            "package": {"ecosystem": "RubyGems", "name": "rack"},
            "ranges": [{"type": "ECOSYSTEM", "events": [{"introduced": "2.0.0"}, {"fixed": "2.2.8"}]}]
        }]
    }]"#;

    fn pinned_harness(name: &str) -> Harness {
        let harness = consumer_harness(name, PINNED_TOML, false);
        harness.write_source(
            "apps/rb/Gemfile",
            "source \"https://rubygems.org\"\ngem \"rack\"\n",
        );
        harness.write_source("apps/rb/Gemfile.lock", GEMFILE_LOCK);
        harness.write_source(
            "tools/ps/PSGallery.requirements.psd1",
            "@{ Pester = '5.7.1' }\n",
        );
        harness.write_source("tools/ps/PSGallery.lock.json", PSGALLERY_LOCK);
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
    fn consumer_powershell_set_with_modules_needs_an_uncovered_acceptance() {
        let harness = pinned_harness("powershell-uncovered");
        let runner = clean_runner();
        let (code, out, err) =
            run_with(&["security", "--output=json", "modules"], &runner, &harness);
        assert_ne!(code, 0, "{out}{err}");
        assert!(out.contains("failed to assess modules"), "{out}{err}");
        assert!(out.contains("no advisory backend"), "{out}{err}");
        assert!(!out.contains("audit_security_clean"), "{out}");
        harness.write_source(
            "security.toml",
            "schema_version = 1\n\n[[uncovered]]\nset = \"modules\"\n\
             reason = \"No Gallery advisory source.\"\nexpires = \"2999-01-01\"\n",
        );
        let (code, out, err) =
            run_with(&["security", "--output=json", "modules"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("failed to assess modules"), "{out}");
    }

    #[test]
    fn consumer_pinned_license_uses_the_ecosystem_inventory_keys() {
        let harness = pinned_harness("pinned-license");
        let runner = clean_runner();
        let (_, out, err) = run_with(&["license", "--output=json"], &runner, &harness);
        assert!(out.contains("rack"), "{out}{err}");
        assert!(out.contains("Pester"), "{out}{err}");
        assert!(out.contains("UNKNOWN"), "{out}");
        assert!(!out.contains("failed to assess"), "{out}");
        harness.write_source(
            "licenses.toml",
            "[policy.distributed]\nallow = [\"MIT\", \"Apache-2.0\"]\nreview = []\ndeny = []\n\n\
             [[inventory]]\npackage = \"rack\"\nset = \"ruby\"\nlicense = \"MIT\"\n\
             versions = \"2.2.0\"\ntext_present = true\n\n\
             [[inventory]]\npackage = \"Pester\"\nset = \"powershell\"\nlicense = \"Apache-2.0\"\n\
             versions = \"5.7.1\"\ntext_present = true\n",
        );
        let (code, out, err) = run_with(&["license", "--output=json"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!out.contains("UNKNOWN"), "{out}");
    }
}
