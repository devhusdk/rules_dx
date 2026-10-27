use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const DIRTY: &str = "import QtQuick\nItem {   property string greet:\"hello\"\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")
load("//ui:qml.bzl", "qml_sources")

quality_family(
    name = "qml_family",
    family_id = "qml",
    disabled = [
        "audit",
        "lint",
        "typecheck",
    ],
    format = ["qmlformat"],
)

workspace_policy(
    name = "policy",
    families = [":qml_family"],
)

qml_sources(
    name = "ui",
    srcs = ["//ui:Main.qml"],
)
"#;

const QML_RULE: &str = r#"load("@rules_dx//quality:sources.bzl", "QualitySourcesInfo")

def _impl(ctx):
    return [
        DefaultInfo(files = depset(ctx.files.srcs)),
        QualitySourcesInfo(direct_sources = {"qml": depset(ctx.files.srcs)}),
    ]

qml_sources = rule(
    implementation = _impl,
    attrs = {"srcs": attr.label_list(allow_files = [".qml"])},
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("quality/tests/fixtures/qmlformat/Sample.qml");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n\nqt_sdk = use_extension(\"@rules_dx//qt/sdk:repos.bzl\", \"qt_sdk\")\nqt_sdk.configure(modules = [\"qtdeclarative\"])\nuse_repo(qt_sdk, \"dx_qt\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(
        &root.join("ui/BUILD.bazel"),
        "exports_files([\"Main.qml\"])\n",
    );
    write(&root.join("ui/qml.bzl"), QML_RULE);
    write(&root.join("ui/Main.qml"), DIRTY);
    root
}

fn dx_with(root: &Case, args: &[&str], tool: bool) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["format"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//...",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    if tool {
        argv.push("--@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat");
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, true)
}

#[test]
fn dirty_check_apply_clean_check_round_trip_in_a_consumer_workspace() {
    let root = consumer();
    let source = root.join("ui/Main.qml");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty
            .stdout
            .contains("ui/Main.qml:1:1: warning: file is not formatted [qmlformat]"),
        "{}",
        dirty.combined()
    );
    assert_eq!(std::fs::read_to_string(&source).expect("read"), DIRTY);

    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let formatted = std::fs::read_to_string(&source).expect("read");
    assert_ne!(formatted, DIRTY);
    assert!(
        formatted.contains("property string greet: \"hello\""),
        "{formatted}"
    );

    let clean = dx(&root, &[]);
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());
    assert_eq!(std::fs::read_to_string(&source).expect("read"), formatted);
}

#[test]
fn malformed_source_fails_and_stays_untouched() {
    let root = consumer();
    let source = root.join("ui/Main.qml");
    let malformed = "import QtQuick\nItem {\n    property string greet: \n";
    write(&source, malformed);

    for args in [&[][..], &["--apply"][..]] {
        let out = dx(&root, args);
        assert_ne!(out.status.code(), Some(0), "{}", out.combined());
        assert_eq!(std::fs::read_to_string(&source).expect("read"), malformed);
    }
}

#[test]
fn bound_qmlformat_ini_changes_the_formatting() {
    let root = consumer();
    write(
        &root.join("ui/.qmlformat.ini"),
        "[General]\nIndentWidth=2\n",
    );
    write(
        &root.join("ui/BUILD.bazel"),
        "load(\"@rules_dx//quality:native_config.bzl\", \"qmlformat_config\")\n\nexports_files([\"Main.qml\"])\n\nqmlformat_config(\n    name = \"qmlformat_ini\",\n    src = \".qmlformat.ini\",\n    visibility = [\"//visibility:public\"],\n)\n",
    );
    write(
        &root.join("BUILD.bazel"),
        &BUILD.replace(
            "    srcs = [\"//ui:Main.qml\"],\n",
            "    srcs = [\"//ui:Main.qml\"],\n    aspect_hints = [\"//ui:qmlformat_ini\"],\n",
        ),
    );
    let applied = dx(&root, &["--apply"]);
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let formatted = std::fs::read_to_string(root.join("ui/Main.qml")).expect("read");
    assert!(
        formatted.contains("\n  property string greet"),
        "{formatted}"
    );
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer();
    let out = dx_with(&root, &[], false);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined()
            .contains("--@rules_dx//config:tool_qmlformat"),
        "{}",
        out.combined()
    );
    assert_eq!(
        std::fs::read_to_string(root.join("ui/Main.qml")).expect("read"),
        DIRTY
    );
}
