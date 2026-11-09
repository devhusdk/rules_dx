//! Re-pins a consumer's Maven lock inside its materialized workspace (manual, needs network).
//!
//! Writes the new lock to the undeclared outputs and fails while the committed lock is stale.

use std::path::PathBuf;

#[test]
fn committed_maven_lock_matches_a_fresh_pin() {
    let rel =
        std::env::var("DX_HERMETIC_MANIFEST").expect("DX_HERMETIC_MANIFEST names the manifest");
    let text = std::fs::read_to_string(dx_testing::resolve_runfiles(&rel)).expect("manifest reads");
    let base = std::env::current_dir().expect("runfiles cwd");
    let manifest = hermetic::parse_manifest(&text, &base).expect("manifest parses");
    let committed = manifest
        .files
        .iter()
        .find(|(_, dest)| dest == "maven_install.json")
        .map(|(src, _)| dx_testing::read_text(src))
        .expect("the manifest maps a maven_install.json");
    let source = hermetic::source_tree(&manifest).expect("source tree");

    let case = dx_nested::case("cpp_rust_pin");
    hermetic::materialize(&manifest, case.root(), &source, "cc").expect("materializes");
    let run = case.run(&case.wrapper(), &["run", "@maven//:pin"], &[("REPIN", "1")]);
    let log = run.combined();
    assert!(run.status.success(), "{log}");

    let pinned = dx_testing::read_text(&case.root().join("maven_install.json"));
    if let Ok(out) = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).expect("outputs dir");
        std::fs::write(out.join("maven_install.json"), &pinned).expect("write pinned lock");
    }
    hermetic::lock_drift(&committed, &pinned).unwrap_or_else(|err| panic!("{err}"));
}
