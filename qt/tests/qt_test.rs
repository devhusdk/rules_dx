use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;

const VERSION: &str = "6.11.3";

const LIBRARY_LICENSE: &str =
    "LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only";

const TOOL_LICENSE: &str = "LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0";

const HOST_GLIBC: [&str; 7] = [
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
];

const HOST_PREFIXES: [&str; 4] = ["/usr/", "/lib/", "/lib64/", "/opt/"];

const HOST_ROUTES: [(&str, &str); 4] = [
    (
        "LD_LIBRARY_PATH",
        "/usr/lib/x86_64-linux-gnu:/usr/lib64:/lib64",
    ),
    ("QML_IMPORT_PATH", "/usr/lib/x86_64-linux-gnu/qt6/qml"),
    ("QT_PLUGIN_PATH", "/usr/lib/x86_64-linux-gnu/qt6/plugins"),
    ("QT_CONF_PATH", "/etc/xdg/qt.conf"),
];

const UNFORMATTED: &str = "import QtQml\nQtObject {   property int a:1\n}\n";

const FORMATTED: &str = "import QtQml\n\nQtObject {\n    property int a: 1\n}\n";

const UNQUALIFIED: &str = "import QtQml\n\nQtObject {\n    property int a: b\n}\n";

fn tool(variable: &str) -> PathBuf {
    let rel = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must name a tool"));
    dx_testing::resolve_runfiles(&rel)
}

fn scratch_file(name: &str, text: &str) -> PathBuf {
    let dir = dx_testing::mkscratch("qt").expect("scratch directory");
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write QML source");
    path
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn host_leaks(paths: &BTreeSet<String>) -> Vec<String> {
    paths
        .iter()
        .filter(|path| HOST_PREFIXES.iter().any(|prefix| path.starts_with(prefix)))
        .filter(|path| !HOST_GLIBC.contains(&file_name(path)))
        .cloned()
        .collect()
}

fn initialized_objects(stderr: &str) -> BTreeSet<String> {
    stderr
        .lines()
        .filter_map(|line| line.split_once("calling init: "))
        .map(|(_, path)| path.trim().to_owned())
        .collect()
}

fn loaded(paths: &BTreeSet<String>, name: &str) -> bool {
    paths.iter().any(|path| file_name(path).starts_with(name))
}

fn lint(file: &Path, envs: &[(&str, &str)]) -> dx_testing::Run {
    let file = file.to_str().expect("utf-8 path");
    dx_testing::run(&tool("DX_QMLLINT"), &["-W", "0", "--json", "-", file], envs)
        .expect("run qmllint")
}

fn spdx_package(document: &Value, name: &str) -> Value {
    document["packages"]
        .as_array()
        .expect("SPDX packages")
        .iter()
        .find(|package| package["name"] == name)
        .unwrap_or_else(|| panic!("SPDX document names no package {name}"))
        .clone()
}

#[test]
fn probe_evaluates_qml_against_only_pinned_libraries() {
    let probe = tool("DX_QT_PROBE");
    let envs = &HOST_ROUTES[1..];
    let run = dx_testing::run(&probe, &[], envs).expect("run probe");
    assert!(run.status.success(), "probe failed:\n{}", run.combined());
    assert!(
        run.stdout
            .starts_with(&format!("Qt {VERSION} evaluated QML with ")),
        "probe output:\n{}",
        run.stdout
    );
    let objects: BTreeSet<String> = run.stdout.lines().skip(1).map(str::to_owned).collect();
    assert_eq!(host_leaks(&objects), Vec::<String>::new());
    for name in [
        "libQt6Qml.so.6",
        "libqmlplugin.so",
        "libicuuc.so.73",
        "libglib-2.0.so.0",
    ] {
        assert!(loaded(&objects, name), "{name} not loaded:\n{}", run.stdout);
    }
}

#[test]
fn tools_report_the_pinned_version() {
    for (variable, name) in [("DX_QMLFORMAT", "qmlformat"), ("DX_QMLLINT", "qmllint")] {
        let run = dx_testing::run(&tool(variable), &["--version"], &[]).expect("run tool");
        assert!(run.status.success(), "{name} failed:\n{}", run.combined());
        assert_eq!(run.stdout, format!("{name} {VERSION}\n"));
    }
}

#[test]
fn qmlformat_rewrites_to_the_qt_layout() {
    let file = scratch_file("format.qml", UNFORMATTED);
    let run = dx_testing::run(
        &tool("DX_QMLFORMAT"),
        &[file.to_str().expect("utf-8 path")],
        &[],
    )
    .expect("run qmlformat");
    assert!(
        run.status.success(),
        "qmlformat failed:\n{}",
        run.combined()
    );
    assert_eq!(run.stdout, FORMATTED);
}

#[test]
fn qmllint_reports_clean_files_as_json() {
    let file = scratch_file("clean.qml", FORMATTED);
    let run = lint(&file, &[]);
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let report: Value = dx_testing::serde_json::from_str(&run.stdout).expect("JSON report");
    assert_eq!(report["revision"], 4);
    assert_eq!(report["files"][0]["success"], true);
    assert_eq!(report["files"][0]["warnings"], Value::Array(Vec::new()));
}

#[test]
fn qmllint_fails_unqualified_access_with_a_located_warning() {
    let file = scratch_file("dirty.qml", UNQUALIFIED);
    let run = lint(&file, &[]);
    assert_eq!(run.status.code(), Some(255), "{}", run.combined());
    let report: Value = dx_testing::serde_json::from_str(&run.stdout).expect("JSON report");
    let warning = &report["files"][0]["warnings"][0];
    assert_eq!(report["files"][0]["success"], false);
    assert_eq!(warning["id"], "unqualified");
    assert_eq!(warning["line"], 4);
    assert_eq!(warning["column"], 21);
}

#[test]
fn tools_ignore_host_routes_and_load_pinned_plugins() {
    let file = scratch_file("plugins.qml", UNQUALIFIED);
    let mut envs = vec![("LD_DEBUG", "files")];
    envs.extend(HOST_ROUTES);
    let run = lint(&file, &envs);
    assert_eq!(run.status.code(), Some(255), "{}", run.stdout);
    let objects = initialized_objects(&run.stderr);
    assert_eq!(host_leaks(&objects), Vec::<String>::new());
    for name in [
        "libQt6QmlCompiler.so.6",
        "libquicklintplugin.so",
        "libicuuc.so.73",
        "libpcre.so.1",
        "libstdc++.so.6",
        "libgcc_s.so.1",
    ] {
        assert!(loaded(&objects, name), "{name} not loaded: {objects:?}");
    }
    assert!(
        objects
            .iter()
            .any(|path| path.ends_with("/plugins/qmllint/libquicklintplugin.so")),
        "{objects:?}"
    );
}

#[test]
fn licenses_cover_qt_icu_and_every_runtime_package() {
    let paths = std::env::var("DX_QT_LICENSES").expect("DX_QT_LICENSES");
    let mut owners = BTreeSet::new();
    for rel in paths.split_whitespace() {
        let (_, tail) = rel.split_once("/licenses/").expect("license path");
        let owner = tail.split('/').next().expect("license owner");
        owners.insert(owner.to_owned());
        let text = std::fs::read(dx_testing::resolve_runfiles(rel)).expect("license text");
        assert!(!text.is_empty(), "{rel} is empty");
    }
    let expected: BTreeSet<String> = [
        "glib2",
        "icu",
        "keyutils-libs",
        "krb5-libs",
        "libbrotli",
        "libcom_err",
        "libgcc",
        "libselinux",
        "libstdc++",
        "libzstd",
        "openssl-libs",
        "pcre",
        "pcre2",
        "qt",
        "zlib",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(owners, expected);
}

#[test]
fn sbom_declares_the_pinned_licenses() {
    let paths = std::env::var("DX_QT_SBOM").expect("DX_QT_SBOM");
    let mut documents = std::collections::BTreeMap::new();
    for rel in paths.split_whitespace() {
        let document = dx_testing::read_json(&dx_testing::resolve_runfiles(rel)).expect("SBOM");
        documents.insert(file_name(rel).to_owned(), document);
    }
    let base = &documents[&format!("qtbase-{VERSION}.spdx.json")];
    let declarative = &documents[&format!("qtdeclarative-{VERSION}.spdx.json")];
    for (document, name, license) in [
        (base, "Core", LIBRARY_LICENSE),
        (declarative, "Qml", LIBRARY_LICENSE),
        (declarative, "qmlformat", TOOL_LICENSE),
        (declarative, "qmllint", TOOL_LICENSE),
    ] {
        let package = spdx_package(document, name);
        assert_eq!(package["versionInfo"], VERSION, "{name}");
        assert_eq!(package["licenseDeclared"], license, "{name}");
    }
}

#[test]
fn launcher_fails_explicitly_without_the_pinned_runfiles() {
    let launcher = std::fs::canonicalize(tool("DX_QMLLINT")).expect("launcher");
    let copy = dx_testing::mkscratch("qt-launcher")
        .expect("scratch directory")
        .join("qmllint");
    std::fs::copy(&launcher, &copy).expect("copy launcher");
    let output = std::process::Command::new(&copy)
        .arg("--version")
        .env_remove("RUNFILES_DIR")
        .env_remove("TEST_SRCDIR")
        .env_remove("RUNFILES_MANIFEST_FILE")
        .output()
        .expect("run launcher copy");
    assert_eq!(output.status.code(), Some(127));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("rules_dx: cannot find the Qt runfiles of ")
            && stderr.ends_with(" for bin/qmllint\n"),
        "{stderr}"
    );
}
