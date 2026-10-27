const DECLARED: &[&str] = &[
    "apple/bundle/defs.bzl",
    "apple/provisioning/defs.bzl",
    "config/BUILD.bazel",
    "deploy/rules/archive.bzl",
    "qt/rules/defs.bzl",
    "qt/sdk/launcher.c",
];

#[test]
fn area_module_srcs_carry_the_sources_nested_consumers_read() {
    for rel in DECLARED {
        let path = dx_testing::resolve_runfiles(rel);
        assert!(path.is_file(), "{rel} is not a file at {path:?}");
    }
}

#[test]
fn rules_dx_source_names_the_checkout() {
    let source = dx_testing::rules_dx_source();
    assert!(source.join("MODULE.bazel").is_file(), "{source:?}");
}
