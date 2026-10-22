use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

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

const ASPECTS: [&str; 16] = [
    "cc_lint",
    "lint",
    "js_lint",
    "python_lint",
    "jvm_lint",
    "rust_lint",
    "shell_lint",
    "text_lint",
    "cc_format",
    "format",
    "js_format",
    "jvm_format",
    "rust_format",
    "shell_format",
    "typecheck",
    "rust_typecheck",
];

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

fn aspects_bzl() -> String {
    let loads: Vec<String> = ASPECTS
        .iter()
        .map(|name| format!("_{name} = \"real_{name}_aspect\""))
        .collect();
    let mut text = format!(
        "load(\"@rules_dx//quality:real_aspects.bzl\", {})\n\n",
        loads.join(", ")
    );
    for name in ASPECTS {
        text.push_str(&format!("real_{name}_aspect = _{name}\n"));
    }
    text
}

fn consumer() -> PathBuf {
    let rules = rules_dx_root();
    let root = mkscratch("qml-consumer").unwrap_or_else(|error| panic!("scratch: {error}"));
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
    write(&root.join("quality/BUILD.bazel"), "");
    write(&root.join("quality/real_aspects.bzl"), &aspects_bzl());
    write(&root.join("ui/BUILD.bazel"), "exports_files([\"Main.qml\"])\n");
    write(&root.join("ui/qml.bzl"), QML_RULE);
    write(&root.join("ui/Main.qml"), DIRTY);
    root
}

fn dx(root: &Path, args: &[&str]) -> Run {
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
        "--@rules_dx//config:tool_qmlformat=@dx_qt//:qmlformat",
    ]);
    run(&resolve_runfiles(&rel), &argv, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
}

#[test]
fn dirty_check_apply_clean_check_round_trip_in_a_consumer_workspace() {
    let root = consumer();
    let source = root.join("ui/Main.qml");

    let dirty = dx(&root, &[]);
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty.stdout.contains("ui/Main.qml:1:1: warning: file is not formatted [qmlformat]"),
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
