use dx_test_scratch::scratch;

mod support;

use support::{commit, dx_in, fixture, write};

const ROOT_CONSTRAINTS: &str = r#"constraint_setting(name = "never", visibility = ["//visibility:public"])

constraint_value(name = "never_on", constraint_setting = ":never", visibility = ["//visibility:public"])
"#;

const D_BUILD: &str = r#"load("//:defs.bzl", "custom_test")

custom_test(name = "d_ok_test", data = ["d.txt"])

custom_test(name = "d_never_test", data = ["d.txt"], target_compatible_with = ["//:never_on"])
"#;

#[test]
fn affected_skips_targets_incompatible_with_the_host() {
    let root_dir = scratch("dx-affected-incompatible");
    let root = root_dir.path();
    fixture(root);
    write(
        &root.join(".gitignore"),
        "/bazel-*\n/MODULE.bazel.lock\n/.dx/\n",
    );
    write(&root.join("BUILD.bazel"), ROOT_CONSTRAINTS);
    write(&root.join("d/BUILD.bazel"), D_BUILD);
    write(&root.join("d/d.txt"), "d\n");
    commit(root);
    write(&root.join("d/d.txt"), "changed\n");

    let built = dx_in(root, &["build", "--affected"]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());

    let ran = dx_in(root, &["test", "--affected"]);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    let all = ran.combined();
    assert!(all.contains("//d:d_ok_test"), "{all}");
    assert!(
        !all.contains("is incompatible and cannot be built"),
        "{all}"
    );

    let json = dx_in(root, &["test", "--affected", "--output=json"]);
    assert_eq!(json.status.code(), Some(0), "{}", json.combined());
    let skipped = json
        .stdout
        .lines()
        .find(|line| line.contains("\"test_outcome\"") && line.contains("\"//d:d_never_test\""))
        .unwrap_or_else(|| panic!("{}", json.combined()));
    assert!(skipped.contains("\"outcome\":\"skipped\""), "{skipped}");

    let strict = dx_in(
        root,
        &[
            "test",
            "--affected",
            "--",
            "--noskip_incompatible_explicit_targets",
        ],
    );
    assert_ne!(strict.status.code(), Some(0), "{}", strict.combined());
    let strict_all = strict.combined();
    assert!(strict_all.contains("//d:d_never_test"), "{strict_all}");
    assert!(
        strict_all.contains("is incompatible and cannot be built"),
        "{strict_all}"
    );
}
