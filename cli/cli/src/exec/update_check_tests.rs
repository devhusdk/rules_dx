use super::test_support::*;

#[test]
fn update_without_dependency_sets_reports_none() {
    let harness = Harness::new("update-no-sets");
    let (code, _, err) = harness.run(&["update"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no dependency sets"), "{err}");
}

#[test]
fn update_names_a_manifest_without_a_lock() {
    let harness = Harness::new("update-unlocked");
    harness.write_source(
        dx_ecosystems::Ecosystem::Cargo.manifest_name(),
        "[package]\nname = \"app\"\n",
    );
    let (code, _, err) = harness.run(&["update"]);
    assert_eq!(code, 2, "{err}");
    assert!(
        err.contains(&format!(
            "no lock yet for {}; run dx generate --apply",
            dx_ecosystems::Ecosystem::Cargo.manifest_name()
        )),
        "{err}"
    );
}

#[test]
fn update_ignores_a_stray_dx_toml() {
    let harness = Harness::new("update-stray-dx-toml");
    harness.write_source("dx.toml", "[[dependency_set]]\nname = \"x\"\nnot toml {{");
    let (code, _, err) = harness.run(&["update"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("no dependency sets"), "{err}");
}
