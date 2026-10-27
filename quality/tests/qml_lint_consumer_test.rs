use std::path::{Path, PathBuf};

use dx_nested::{self as nested, Case};
use dx_testing::{resolve_runfiles, Run};

const MAIN: &str = "import QtQuick\nimport Foo\nimport Vendor\n\nItem {\n    Bar {\n        id: bar\n    }\n    Thing {\n        id: thing\n    }\n    property int total: bar.value + thing.value\n}\n";

const DIRTY: &str = "import QtQuick\nimport Foo\nimport Nope.Missing\n\nItem {\n    Missing {}\n    property string s: foo\n    property int u: \"\u{1F600}\" + bar\n}\n";

const BUILD: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "qml_family",
    family_id = "qml",
    disabled = [
        "audit",
        "format",
        "typecheck",
    ],
    lint = ["qmllint"],
)

workspace_policy(
    name = "policy",
    families = [":qml_family"],
)
"#;

const QML_RULE: &str = r#"load("@rules_dx//quality:sources.bzl", "QualitySourcesInfo")

def _impl(ctx):
    owned = [f for f in ctx.files.srcs if f.extension == "qml"]
    return [
        DefaultInfo(files = depset(ctx.files.srcs)),
        QualitySourcesInfo(direct_sources = {"qml": depset(owned)}),
    ]

qml_module = rule(
    implementation = _impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
    },
)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rules_dx_root() -> PathBuf {
    let sample = resolve_runfiles("quality/tests/fixtures/qmllint/Sample.qml");
    let real = std::fs::canonicalize(&sample).unwrap_or_else(|error| panic!("{sample:?}: {error}"));
    real.ancestors()
        .nth(5)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn module(root: &Path, dir: &str, name: &str, uri: &str, type_name: &str, body: &str) {
    write(
        &root.join(dir).join("qmldir"),
        &format!("module {uri}\n{type_name} 1.0 {type_name}.qml\n"),
    );
    write(&root.join(dir).join(format!("{type_name}.qml")), body);
    write(
        &root.join(dir).join("BUILD.bazel"),
        &format!(
            "load(\"//:qml.bzl\", \"qml_module\")\n\nqml_module(\n    name = \"{name}\",\n    srcs = [\"qmldir\", \"{type_name}.qml\"],\n    visibility = [\"//visibility:public\"],\n)\n"
        ),
    );
}

fn app_build(deps: &str, hints: &str) -> String {
    format!(
        "load(\"//:qml.bzl\", \"qml_module\")\n\nqml_module(\n    name = \"app\",\n    srcs = [\"Main.qml\"],\n    deps = [{deps}],\n{hints})\n"
    )
}

const DEPS: &str = "\"//mods/Foo:foo\", \"//third_party/Vendor:vendor\"";

fn consumer() -> Case {
    let rules = rules_dx_root();
    let root = nested::case("consumer");
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_rust\", version = \"0.74.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n\nqt_sdk = use_extension(\"@rules_dx//qt/sdk:repos.bzl\", \"qt_sdk\")\nqt_sdk.configure(modules = [\"qtdeclarative\"])\nuse_repo(qt_sdk, \"dx_qt\")\n",
            rules.display()
        ),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), BUILD);
    write(&root.join("qml.bzl"), QML_RULE);
    module(
        &root,
        "mods/Foo",
        "foo",
        "Foo",
        "Bar",
        "import QtQuick\nItem {\n    property int value: 1\n}\n",
    );
    module(
        &root,
        "third_party/Vendor",
        "vendor",
        "Vendor",
        "Thing",
        "import QtQuick\nItem {\n    property int value: 2\n    property int bad: vendored\n}\n",
    );
    write(&root.join("app/Main.qml"), MAIN);
    write(&root.join("app/BUILD.bazel"), &app_build(DEPS, ""));
    root
}

fn dx_with(root: &Case, args: &[&str], tool: bool, bazel: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec!["lint"];
    argv.extend_from_slice(args);
    argv.extend_from_slice(&[
        "--workspace",
        workspace.as_str(),
        "//app:app",
        "--",
        "--@rules_dx//config:workspace=//:policy",
    ]);
    if tool {
        argv.push("--@rules_dx//config:tool_qmllint=@dx_qt//:qmllint");
    }
    argv.extend_from_slice(bazel);
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn dx(root: &Case, args: &[&str]) -> Run {
    dx_with(root, args, true, &[])
}

#[test]
fn clean_multi_module_consumer_passes() {
    let root = consumer();
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
    assert!(!out.combined().contains("Vendor"), "{}", out.combined());
}

#[test]
fn deliberate_errors_are_reported_with_locations_and_third_party_is_not_linted() {
    let root = consumer();
    write(&root.join("app/Main.qml"), DIRTY);
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    let text = out.stdout.as_str();
    assert!(
        text.contains("app/Main.qml:7:24: warning: Unqualified access [qmllint/unqualified]"),
        "{}",
        out.combined()
    );
    assert!(
        text.contains("app/Main.qml:3:1: warning: Failed to import Nope.Missing"),
        "{}",
        out.combined()
    );
    assert!(
        text.contains("app/Main.qml:6:5: warning: Missing was not found"),
        "{}",
        out.combined()
    );
    assert!(
        text.contains("app/Main.qml:8:27: warning: Unqualified access [qmllint/unqualified]"),
        "{}",
        out.combined()
    );
    assert!(
        text.contains("app/Main.qml:2:1: info: Unused import [qmllint/unused-imports]"),
        "{}",
        out.combined()
    );
    assert!(!text.contains("Vendor.qml"), "{}", out.combined());
    assert!(!text.contains("vendored"), "{}", out.combined());
}

#[test]
fn transitive_deps_resolve_without_forwarded_files() {
    let root = consumer();
    module(
        &root,
        "mods/Baz",
        "baz",
        "Baz",
        "Quux",
        "import QtQuick\nItem {\n    property int value: 3\n}\n",
    );
    write(
        &root.join("mods/Foo/BUILD.bazel"),
        "load(\"//:qml.bzl\", \"qml_module\")\n\nqml_module(\n    name = \"foo\",\n    srcs = [\"qmldir\", \"Bar.qml\"],\n    deps = [\"//mods/Baz:baz\"],\n    visibility = [\"//visibility:public\"],\n)\n",
    );
    write(
        &root.join("app/Main.qml"),
        "import QtQuick\nimport Baz\nItem {\n    Quux {\n    }\n}\n",
    );
    write(
        &root.join("app/BUILD.bazel"),
        &app_build("\"//mods/Foo:foo\"", ""),
    );
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn external_repository_modules_resolve() {
    let root = consumer();
    let module_file = root.join("MODULE.bazel");
    let text = std::fs::read_to_string(&module_file).expect("module file");
    write(
        &module_file,
        &format!(
            "{text}\nlocal_repository = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:local.bzl\", \"local_repository\")\n\nlocal_repository(\n    name = \"ext_qml\",\n    path = \"ext\",\n)\n"
        ),
    );
    write(&root.join("ext/REPO.bazel"), "");
    write(&root.join("ext/BUILD.bazel"), "");
    write(&root.join("ext/qml.bzl"), QML_RULE);
    write(
        &root.join("ext/Ext/qmldir"),
        "module Ext\nGadget 1.0 Gadget.qml\n",
    );
    write(
        &root.join("ext/Ext/Gadget.qml"),
        "import QtQuick\nItem {\n    property int value: 4\n}\n",
    );
    write(
        &root.join("ext/Ext/BUILD.bazel"),
        "load(\"//:qml.bzl\", \"qml_module\")\n\nqml_module(\n    name = \"ext\",\n    srcs = [\"qmldir\", \"Gadget.qml\"],\n    visibility = [\"//visibility:public\"],\n)\n",
    );
    write(
        &root.join("app/Main.qml"),
        "import QtQuick\nimport Ext\nItem {\n    Gadget {\n    }\n}\n",
    );
    write(
        &root.join("app/BUILD.bazel"),
        &app_build("\"@ext_qml//Ext:ext\"", ""),
    );
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn undeclared_modules_do_not_resolve_even_with_a_host_import_path() {
    let root = consumer();
    write(&root.join("app/BUILD.bazel"), &app_build("", ""));
    let imports = format!(
        "--action_env=QML_IMPORT_PATH={}",
        root.join("mods").display()
    );
    let out = dx_with(&root, &[], true, &[imports.as_str()]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        out.stdout.contains("Failed to import Foo"),
        "{}",
        out.combined()
    );
    assert!(
        out.stdout.contains("Failed to import Vendor"),
        "{}",
        out.combined()
    );
}

#[test]
fn bound_qmllint_ini_changes_the_findings() {
    let root = consumer();
    write(&root.join("app/Main.qml"), DIRTY);
    write(
        &root.join("app/lint.qmllint.ini"),
        "[Warnings]\nUnqualifiedAccess=disable\n",
    );
    write(
        &root.join("app/BUILD.bazel"),
        &format!(
            "load(\"@rules_dx//quality:native_config.bzl\", \"qmllint_config\")\n{}\nqmllint_config(\n    name = \"qmllint_ini\",\n    src = \"lint.qmllint.ini\",\n)\n",
            app_build(DEPS, "    aspect_hints = [\":qmllint_ini\"],\n")
        ),
    );
    let out = dx(&root, &[]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    assert!(
        !out.stdout.contains("Unqualified access"),
        "{}",
        out.combined()
    );
    assert!(
        out.stdout.contains("Failed to import Nope.Missing"),
        "{}",
        out.combined()
    );
}

#[test]
fn sarif_report_carries_the_real_findings() {
    let root = consumer();
    write(&root.join("app/Main.qml"), DIRTY);
    let report = root.join("lint.sarif");
    let destination = format!("sarif={}", report.display());
    let out = dx(&root, &["--report", destination.as_str()]);
    assert_eq!(out.status.code(), Some(1), "{}", out.combined());
    let sarif = std::fs::read_to_string(&report).expect("sarif report");
    let doc: dx_testing::serde_json::Value =
        dx_testing::serde_json::from_str(&sarif).expect("sarif json");
    let results = doc["runs"][0]["results"].as_array().expect("results");
    let unqualified = results
        .iter()
        .find(|result| {
            result["ruleId"]
                .as_str()
                .is_some_and(|id| id.ends_with("unqualified"))
        })
        .unwrap_or_else(|| panic!("no unqualified result in {sarif}"));
    let region = &unqualified["locations"][0]["physicalLocation"]["region"];
    assert_eq!(region["startLine"], 7);
    assert_eq!(region["startColumn"], 24);
    assert!(
        unqualified["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("app/Main.qml")),
        "{sarif}"
    );
}

#[test]
fn missing_tool_flag_fails_and_names_the_flag() {
    let root = consumer();
    let out = dx_with(&root, &[], false, &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("--@rules_dx//config:tool_qmllint"),
        "{}",
        out.combined()
    );
}
