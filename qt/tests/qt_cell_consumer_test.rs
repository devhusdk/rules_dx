use std::path::Path;

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, rules_dx_source};

const BUILD: &str = r#"constraint_setting(name = "variant")

constraint_value(
    name = "custom",
    constraint_setting = ":variant",
)

platform(
    name = "custom_platform",
    constraint_values = [":custom"],
    parents = ["@rules_dx//qt/platforms:linux_x86_64"],
)
"#;

const SHA256: &str = "ce282da4d8a83dc216ed96b715202872d655177189c16205dc5fab09b41a9f09";

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn spec() -> String {
    let rel = std::env::var("CELL_SPEC").unwrap_or_else(|_| panic!("CELL_SPEC must name the spec"));
    let path = resolve_runfiles(&rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn consumer(spec: &str) -> Case {
    let rules = rules_dx_source();
    let root = nested::case("cell_consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"cell_consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"platforms\", version = \"1.1.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n\nqt_sdk = use_extension(\"@rules_dx//qt/sdk:repos.bzl\", \"qt_sdk\")\nqt_sdk.configure()\nqt_sdk.cell(\n    name = \"custom_x86_64\",\n    constraints = [\n        \"@platforms//os:linux\",\n        \"@platforms//cpu:x86_64\",\n        \"//:custom\",\n    ],\n    spec = \"//:custom_x86_64.json\",\n)\nuse_repo(qt_sdk, \"dx_qt\")\n",
            rules.display()
        ),
    );
    write(
        &root.join(".bazelrc"),
        "common --enable_bzlmod\nbuild --platforms=//:custom_platform\nbuild --remote_download_outputs=all\n",
    );
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("custom_x86_64.json"), spec);
    root
}

#[test]
fn a_consumer_cell_serves_dx_qt_core() {
    let root = consumer(&spec());
    let built = root.bazel(&["build", "@dx_qt//:core"]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());

    let deps = root.bazel(&["cquery", "deps(@dx_qt//:core, 1)"]);
    assert_eq!(deps.status.code(), Some(0), "{}", deps.combined());
    assert!(
        deps.stdout.contains("dx_qt_custom_x86_64//:core"),
        "{}",
        deps.combined()
    );
    assert!(
        !deps.stdout.contains("dx_qt_linux_x86_64//:core"),
        "{}",
        deps.combined()
    );
}

#[test]
fn a_consumer_cell_with_a_bad_sha256_fails() {
    let text = spec();
    assert!(text.contains(SHA256), "{text}");
    let root = consumer(&text.replace(SHA256, "not-a-sha256"));
    let built = root.bazel(&["build", "@dx_qt//:core"]);
    assert_ne!(built.status.code(), Some(0), "{}", built.combined());
    assert!(
        built
            .combined()
            .contains("cell 'custom_x86_64' archive 'qtbase' sha256 is not 64 hex characters"),
        "{}",
        built.combined()
    );
}
