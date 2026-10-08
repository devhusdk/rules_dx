#[cfg(test)]
mod tests {
    use super::super::super::test_support::Harness;
    use super::super::super::test_support::{event_kinds, json_events};
    use super::super::audit_live::AuditRunner;

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

    #[test]
    fn consumer_bare_security_reports_coverage_gap_and_no_phantoms() {
        let harness = consumer_harness("bare", DX_TOML, true);
        let runner = clean_runner();
        let (code, out, err) = run_with(&["security", "--output=json"], &runner, &harness);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("audit_security_clean"), "{out}");
        assert!(
            out.contains("no advisory coverage for frontend, worker"),
            "{out}"
        );
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
    fn consumer_name_and_scope_select_the_same_set() {
        let harness = consumer_harness("select", DX_TOML, true);
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "frontend"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("no advisory coverage for frontend\""), "{out}");
        assert!(!out.contains("worker"), "{out}");
        let runner = clean_runner();
        let (code, out, err) = run_with(
            &["security", "--output=json", "services/worker/jobs"],
            &runner,
            &harness,
        );
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("no advisory coverage for worker"), "{out}");
        assert!(!out.contains("frontend"), "{out}");
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
}
