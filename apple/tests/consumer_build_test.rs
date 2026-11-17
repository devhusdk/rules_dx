//! The Apple consumer fixture builds its iOS simulator app and its macOS app with no flag.

use std::collections::BTreeMap;

use dx_nested::compose::{
    rules_flutter_override, rules_py_override, write_root_patch_override,
    write_root_patch_overrides, write_rules_py_patches,
};

/// The fixture directory under the source snapshot.
const PREFIX: &str = "apple/tests/consumer/";

/// The name the fixture stores `MODULE.bazel` under.
const STORED_MODULE: &str = "consumer.MODULE.bazel";

/// The files of the fixture, keyed by their path under it.
fn fixture() -> BTreeMap<String, String> {
    let snapshot = dx_testing::rlocation(dx_testing::env_text("DX_CONSUMER_JSON").trim())
        .unwrap_or_else(|error| panic!("DX_CONSUMER_JSON: {error}"));
    let files: BTreeMap<String, String> =
        dx_testing::serde_json::from_str(&dx_testing::read_text(&snapshot))
            .unwrap_or_else(|error| panic!("{}: {error}", snapshot.display()));
    files
        .into_iter()
        .filter_map(|(path, text)| Some((path.strip_prefix(PREFIX)?.to_owned(), text)))
        .collect()
}

/// The fixture module without its `module` and `rules_dx` lines, which the nested consumer writes.
fn module_body(module: &str) -> String {
    module
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with('#') && !line.starts_with("module(") && !line.starts_with("bazel_dep(")
        })
        .map(|line| format!("{line}\n"))
        .collect()
}

#[test]
fn one_macro_call_builds_each_app_with_no_flag() {
    let files = fixture();
    let module = files
        .get(STORED_MODULE)
        .unwrap_or_else(|| panic!("{PREFIX}{STORED_MODULE} is missing from the snapshot"));
    let source = dx_testing::rules_dx_source();
    let case = dx_nested::Case::consumer(
        "apple_consumer",
        &format!(
            "{}{}{}",
            module_body(module),
            rules_py_override(),
            rules_flutter_override(),
        ),
    );
    write_rules_py_patches(case.root(), &source).unwrap_or_else(|error| panic!("{error}"));
    let mut root_overrides =
        write_root_patch_overrides(case.root(), &source).unwrap_or_else(|error| panic!("{error}"));
    for (module, skip) in [
        ("apple_support", &["toolchains_gate.patch"][..]),
        ("rules_swift", &[][..]),
    ] {
        root_overrides.push_str(
            &write_root_patch_override(case.root(), &source, module, skip)
                .unwrap_or_else(|error| panic!("{error}")),
        );
    }
    let composed = dx_testing::read_text(&case.root().join("MODULE.bazel"));
    case.write("MODULE.bazel", &format!("{composed}{root_overrides}"));
    for (rel, text) in &files {
        if rel != STORED_MODULE {
            case.write(rel, text);
        }
    }
    case.use_lock("apple");
    let built = case.bazel(&[
        "build",
        "--color=no",
        "--curses=no",
        "//phone:phone_app",
        "//desk:desk_app",
    ]);
    assert!(built.status.success(), "{}", built.combined());
}
