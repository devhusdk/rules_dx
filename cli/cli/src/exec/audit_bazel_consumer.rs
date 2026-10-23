#[cfg(test)]
mod tests {
    use super::super::super::test_support::Harness;
    use super::super::audit_live::AuditRunner;

    const DX_TOML: &str = r#"
schema_version = 1

[[dependency_set]]
name = "bazel-modules"
ecosystem = "bazel"
manifests = ["MODULE.bazel"]
locks = ["MODULE.bazel.lock"]
scopes = ["."]
"#;

    const MODULE: &str = "bazel_dep(name = \"rules_cc\", version = \"0.2.0\")\n";
    const REGISTRY: &str = "https://bcr.bazel.build";
    const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn lock_for(name: &str, version: &str) -> String {
        format!(
            "{{\"lockFileVersion\": 28, \"registryFileHashes\": {{\"{REGISTRY}/bazel_registry.json\": \"{DIGEST}\", \"{REGISTRY}/modules/{name}/{version}/MODULE.bazel\": \"{DIGEST}\", \"{REGISTRY}/modules/{name}/{version}/source.json\": \"{DIGEST}\"}}}}"
        )
    }

    fn harness(name: &str, module: &str, lock: Option<&str>) -> Harness {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let harness = Harness::new(&format!("audit-bazel-{name}-{id}"));
        harness.write_source("dx.toml", DX_TOML);
        harness.write_source("MODULE.bazel", module);
        if let Some(lock) = lock {
            harness.write_source("MODULE.bazel.lock", lock);
        }
        harness
    }

    fn run(argv: &[&str], harness: &Harness) -> (i32, String, String) {
        let words: Vec<String> = argv.iter().map(|word| (*word).to_owned()).collect();
        let invocation = crate::args::parse(&words).expect("parse");
        harness.execute_with(&invocation, &AuditRunner::clean())
    }

    fn with_sarif(argv: &[&str], h: &Harness) -> (i32, String, String) {
        let mut words = argv.to_vec();
        words.extend_from_slice(&["--report", "sarif=out.sarif"]);
        let (code, out, err) = run(&words, h);
        let sarif = std::fs::read_to_string(h.workspace.join("out.sarif")).unwrap_or_default();
        (code, format!("{out}{sarif}"), err)
    }

    #[test]
    fn security_reports_bazel_inputs_as_not_covered() {
        let h = harness("security", MODULE, Some(&lock_for("rules_cc", "0.2.1")));
        let (code, out, err) = with_sarif(&["security", "--output=json"], &h);
        assert_eq!(code, 0, "{out}{err}");
        assert!(
            out.contains("clean; no advisory coverage for bazel-modules"),
            "{out}"
        );
        assert!(out.contains("1 Bazel inputs not assessed"), "{out}");
        assert!(out.contains("MODULE.bazel.lock"), "{out}");
    }

    #[test]
    fn license_is_unknown_without_a_declared_inventory_entry() {
        let h = harness("unknown", MODULE, Some(&lock_for("rules_cc", "0.2.1")));
        let (code, out, err) = with_sarif(&["license", "--output=json"], &h);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("1 license findings"), "{out}");
        assert!(out.contains("rules_cc"), "{out}");
        assert!(out.contains("UNKNOWN"), "{out}");
    }

    #[test]
    fn license_uses_the_declared_inventory_entry() {
        let h = harness("declared", MODULE, Some(&lock_for("rules_cc", "0.2.1")));
        h.write_source(
            "licenses.toml",
            "[policy]\nblocked = []\n[policy.distributed]\nallow = [\"Apache-2.0\"]\nreview = []\ndeny = []\n[[inventory]]\npackage = \"rules_cc\"\nset = \"bazel\"\nlicense = \"Apache-2.0\"\nversions = \"0.2.1\"\ntext_present = true\n",
        );
        let (code, out, err) = run(&["license", "--output=json"], &h);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("audit_license_clean"), "{out}");
    }

    #[test]
    fn a_missing_lock_is_incomplete() {
        let h = harness("missing", MODULE, None);
        for family in ["security", "license"] {
            let (code, out, err) = run(&[family, "--output=json"], &h);
            assert_ne!(code, 0, "{family}: {out}{err}");
            assert!(
                err.contains("failed to assess bazel-modules"),
                "{family}: {err}"
            );
            assert!(err.contains("MODULE.bazel.lock"), "{family}: {err}");
        }
    }

    #[test]
    fn a_module_missing_from_the_lock_is_incomplete() {
        let h = harness("mismatch", MODULE, Some(&lock_for("platforms", "1.0.0")));
        for family in ["security", "license"] {
            let (code, out, err) = run(&[family, "--output=json"], &h);
            assert_ne!(code, 0, "{family}: {out}{err}");
            assert!(
                err.contains("failed to assess bazel-modules"),
                "{family}: {err}"
            );
            assert!(err.contains("rules_cc"), "{family}: {err}");
        }
    }

    #[test]
    fn update_check_reports_the_set_pinned_and_apply_changes_nothing() {
        let h = harness("update", MODULE, Some(&lock_for("rules_cc", "0.2.1")));
        let (code, out, err) = run(&["update", "--check", "--output=json"], &h);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("bazel-modules"), "{out}");
        let before = std::fs::read_to_string(h.workspace.join("MODULE.bazel.lock")).unwrap();
        let (code, out, err) = run(&["update", "--apply", "--output=json"], &h);
        assert_eq!(code, 0, "{out}{err}");
        let after = std::fs::read_to_string(h.workspace.join("MODULE.bazel.lock")).unwrap();
        assert_eq!(before, after);
    }
}
