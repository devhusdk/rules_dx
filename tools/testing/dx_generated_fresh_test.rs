#[test]
fn dx_build_matches_dx_generate() {
    let root = dx_testing::rules_dx_source();
    let planned = dx_generate::plan(&root, &mut dx_adopt::detected_families)
        .unwrap_or_else(|error| panic!("dx generate: {error}"));
    let want = planned
        .get(".dx/BUILD.bazel")
        .expect("dx generate renders .dx/BUILD.bazel");
    let have = std::fs::read(root.join(".dx/BUILD.bazel")).expect("read .dx/BUILD.bazel");
    assert!(
        *want == have,
        ".dx/BUILD.bazel differs from the dx generate output; run `tools/dx generate --apply`"
    );
}
