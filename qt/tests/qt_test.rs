use dx_testing::env_file;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;

fn version() -> String {
    std::env::var("DX_QT_VERSION").expect("DX_QT_VERSION")
}

const LIBRARY_LICENSE: &str =
    "LicenseRef-Qt-Commercial OR LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only";

const TOOL_LICENSE: &str = "LicenseRef-Qt-Commercial OR GPL-3.0-only WITH Qt-GPL-exception-1.0";

#[cfg(target_os = "linux")]
const HOST_GLIBC: &[&str] = &[
    "ld-linux-aarch64.so.1",
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
];

#[cfg(target_os = "macos")]
const HOST_GLIBC: &[&str] = &[];

#[cfg(windows)]
const HOST_GLIBC: &[&str] = &[];

#[cfg(target_os = "linux")]
const HOST_PREFIXES: &[&str] = &["/usr/", "/lib/", "/lib64/", "/opt/"];

#[cfg(target_os = "macos")]
const HOST_PREFIXES: &[&str] = &["/usr/local/", "/opt/", "/Library/Frameworks/"];

#[cfg(windows)]
const HOST_PREFIXES: &[&str] = &["C:/Qt/", "C:/msys64/", "C:/vcpkg/"];

#[cfg(target_os = "macos")]
const HOST_ROUTES: [(&str, &str); 4] = [
    ("DYLD_FRAMEWORK_PATH", "/opt/homebrew/lib:/usr/local/lib"),
    ("QML_IMPORT_PATH", "/opt/homebrew/share/qt/qml"),
    ("QT_PLUGIN_PATH", "/opt/homebrew/share/qt/plugins"),
    ("QT_CONF_PATH", "/opt/homebrew/bin/qt.conf"),
];

#[cfg(windows)]
const HOST_ROUTES: [(&str, &str); 4] = [
    ("PATH", "C:/Qt/6.11.3/msvc2022_64/bin"),
    ("QML_IMPORT_PATH", "C:/Qt/6.11.3/msvc2022_64/qml"),
    ("QT_PLUGIN_PATH", "C:/Qt/6.11.3/msvc2022_64/plugins"),
    ("QT_CONF_PATH", "C:/Qt/6.11.3/msvc2022_64/bin/qt.conf"),
];

#[cfg(target_os = "linux")]
const LOADER_TRACE: (&str, &str) = ("LD_DEBUG", "files");

#[cfg(target_os = "macos")]
const LOADER_TRACE: (&str, &str) = ("DYLD_PRINT_LIBRARIES", "1");

#[cfg(windows)]
const LOADER_TRACE: (&str, &str) = ("DX_QT_TRACE_MODULES", "1");

#[cfg(target_os = "linux")]
const OFFSCREEN: &str = "libqoffscreen.so";

#[cfg(target_os = "macos")]
const OFFSCREEN: &str = "libqoffscreen.dylib";

#[cfg(windows)]
const OFFSCREEN: &str = "qoffscreen.dll";

#[cfg(target_os = "linux")]
const PROBE_LOADS: &[&str] = &[
    "libQt6Qml.so.6",
    "libqmlplugin.so",
    "libicuuc.so.73",
    "libglib-2.0.so.0",
];

#[cfg(target_os = "macos")]
const PROBE_LOADS: &[&str] = &["QtQml", "libqmlplugin.dylib"];

#[cfg(windows)]
const PROBE_LOADS: &[&str] = &["Qt6Core", "Qt6Qml", "qmlplugin.dll"];

#[cfg(target_os = "linux")]
const GUI_LOADS: &[&str] = &[
    "libQt6Widgets.so.6",
    "libQt6Quick.so.6",
    "libqoffscreen.so",
    "libfontconfig.so.1",
    "libdbus-1.so.3",
    "libsystemd.so.0",
    "libstdc++.so.6",
];

#[cfg(target_os = "macos")]
const GUI_LOADS: &[&str] = &["QtWidgets", "QtQuick", "libqoffscreen.dylib"];

#[cfg(windows)]
const GUI_LOADS: &[&str] = &["Qt6Widgets", "Qt6Quick", "qoffscreen.dll"];

#[cfg(target_os = "linux")]
const LINT_LOADS: &[&str] = &[
    "libQt6QmlCompiler.so.6",
    "libquicklintplugin.so",
    "libicuuc.so.73",
    PCRE,
    "libstdc++.so.6",
    "libgcc_s.so.1",
];

#[cfg(target_os = "macos")]
const LINT_LOADS: &[&str] = &["QtQmlCompiler", "libquicklintplugin.dylib"];

#[cfg(target_os = "linux")]
const LINT_PLUGIN: &str = "/plugins/qmllint/libquicklintplugin.so";

#[cfg(target_os = "macos")]
const LINT_PLUGIN: &str = "/plugins/qmllint/libquicklintplugin.dylib";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const HOST_ROUTES: [(&str, &str); 4] = [
    (
        "LD_LIBRARY_PATH",
        "/usr/lib/aarch64-linux-gnu:/usr/lib64:/lib64",
    ),
    ("QML_IMPORT_PATH", "/usr/lib/aarch64-linux-gnu/qt6/qml"),
    ("QT_PLUGIN_PATH", "/usr/lib/aarch64-linux-gnu/qt6/plugins"),
    ("QT_CONF_PATH", "/etc/xdg/qt.conf"),
];

#[cfg(all(target_os = "linux", not(target_arch = "aarch64")))]
const HOST_ROUTES: [(&str, &str); 4] = [
    (
        "LD_LIBRARY_PATH",
        "/usr/lib/x86_64-linux-gnu:/usr/lib64:/lib64",
    ),
    ("QML_IMPORT_PATH", "/usr/lib/x86_64-linux-gnu/qt6/qml"),
    ("QT_PLUGIN_PATH", "/usr/lib/x86_64-linux-gnu/qt6/plugins"),
    ("QT_CONF_PATH", "/etc/xdg/qt.conf"),
];

#[cfg(target_os = "linux")]
const PCRE: &str = if cfg!(target_arch = "aarch64") {
    "libpcre2-8.so.0"
} else {
    "libpcre.so.1"
};

#[cfg(target_os = "linux")]
const CELL_LICENSES: &[&str] = if cfg!(target_arch = "aarch64") {
    &["libICE", "libSM", "libuuid", "zlib-ng-compat"]
} else {
    &[
        "keyutils-libs",
        "krb5-libs",
        "libcom_err",
        "libgcrypt",
        "libgpg-error",
        "libselinux",
        "lz4-libs",
        "openssl-libs",
        "pcre",
        "zlib",
    ]
};

const UNFORMATTED: &str = "import QtQml\nQtObject {   property int a:1\n}\n";

const FORMATTED: &str = "import QtQml\n\nQtObject {\n    property int a: 1\n}\n";

#[cfg(not(windows))]
const UNQUALIFIED_EXIT: i32 = 255;

#[cfg(windows)]
const UNQUALIFIED_EXIT: i32 = -1;

const UNQUALIFIED: &str = "import QtQml\n\nQtObject {\n    property int a: b\n}\n";

/// Windows searches the executable's directory before System32 and PATH, so the pinned
/// MSVC runtime is copied beside a private copy of the executable.
#[cfg(windows)]
fn runnable(variable: &str) -> (Option<dx_test_scratch::TempDir>, PathBuf) {
    let exe = env_file(variable);
    let scratch = dx_test_scratch::scratch("qt-exe");
    let dir = scratch.path();
    let copy = dir.join(exe.file_name().expect("executable name"));
    std::fs::copy(&exe, &copy).expect("copy executable");
    let paths = std::env::var("DX_QT_VCRUNTIME").expect("DX_QT_VCRUNTIME must name files");
    for path in paths
        .split_whitespace()
        .filter(|path| path.ends_with(".dll"))
    {
        let source = dx_testing::resolve_runfiles(path);
        std::fs::copy(&source, dir.join(file_name(path))).expect("copy MSVC runtime");
    }
    (Some(scratch), copy)
}

#[cfg(not(windows))]
fn runnable(variable: &str) -> (Option<dx_test_scratch::TempDir>, PathBuf) {
    (None, env_file(variable))
}

fn scratch_file(name: &str, text: &str) -> (dx_test_scratch::TempDir, PathBuf) {
    let scratch = dx_test_scratch::scratch("qt");
    let path = scratch.path().join(name);
    std::fs::write(&path, text).expect("write QML source");
    (scratch, path)
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn host_leaks(paths: &BTreeSet<String>) -> Vec<String> {
    paths
        .iter()
        .filter(|path| {
            HOST_PREFIXES.iter().any(|prefix| path.starts_with(prefix)) || host_msvc_runtime(path)
        })
        .filter(|path| !HOST_GLIBC.contains(&file_name(path)))
        .cloned()
        .collect()
}

fn expected_platform_plugins() -> BTreeSet<String> {
    let mut plugins = BTreeSet::from(["minimal".to_owned(), "offscreen".to_owned()]);
    if cfg!(target_os = "macos") {
        plugins.insert("cocoa".to_owned());
    }
    // The Windows archive ships these in plugins/platforms beside the pinned ones.
    if cfg!(windows) {
        plugins.extend(["direct2d".to_owned(), "windows".to_owned()]);
    }
    plugins
}

/// Parses Qt's "Available platform plugins are: a, b." line into a set; empty if absent.
fn available_platform_plugins(output: &str) -> BTreeSet<String> {
    const MARKER: &str = "Available platform plugins are: ";
    output
        .lines()
        .find_map(|line| line.find(MARKER).map(|i| &line[i + MARKER.len()..]))
        .map(|rest| {
            rest.trim()
                .trim_end_matches('.')
                .split(',')
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn initialized_objects(stderr: &str) -> BTreeSet<String> {
    stderr
        .lines()
        .filter_map(|line| line.split_once("calling init: "))
        .map(|(_, path)| path.trim().to_owned())
        .collect()
}

#[cfg(target_os = "macos")]
fn initialized_objects(stderr: &str) -> BTreeSet<String> {
    stderr
        .lines()
        .filter(|line| line.starts_with("dyld["))
        .filter_map(|line| line.split_once("]: "))
        .map(|(_, rest)| {
            let path = rest.trim();
            path.split_once("> ")
                .map_or(path, |(_, path)| path)
                .to_owned()
        })
        .collect()
}

/// gui_test prints one `dx-module: <path>` line per loaded module when DX_QT_TRACE_MODULES is set.
#[cfg(windows)]
fn initialized_objects(stderr: &str) -> BTreeSet<String> {
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix("dx-module: "))
        .map(|path| path.trim().to_owned())
        .collect()
}

fn loaded(paths: &BTreeSet<String>, name: &str) -> bool {
    paths.iter().any(|path| file_name(path).starts_with(name))
}

fn lint(file: &Path, envs: &[(&str, &str)]) -> dx_testing::Run {
    let file = file.to_str().expect("utf-8 path");
    dx_testing::run(
        &env_file("DX_QMLLINT"),
        &["-W", "0", "--json", "-", file],
        envs,
    )
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
    let (_scratch, probe) = runnable("DX_QT_PROBE");
    let path = runtime_path();
    let mut envs = vec![("PATH", path.as_str())];
    envs.extend(&HOST_ROUTES[1..]);
    let run = dx_testing::run(&probe, &[], &envs).expect("run probe");
    assert!(run.status.success(), "probe failed:\n{}", run.combined());
    assert!(
        run.stdout
            .starts_with(&format!("Qt {} evaluated QML with ", version())),
        "probe output:\n{}",
        run.stdout
    );
    let objects: BTreeSet<String> = run.stdout.lines().skip(1).map(str::to_owned).collect();
    assert_eq!(host_leaks(&objects), Vec::<String>::new());
    for name in PROBE_LOADS {
        assert!(loaded(&objects, name), "{name} not loaded:\n{}", run.stdout);
    }
}

/// Windows loads DLLs from next to the symlinked executable, not next to its target,
/// so the pinned runtime `bin` directory goes first on PATH for the children.
#[cfg(windows)]
fn runtime_path() -> String {
    let paths = std::env::var("DX_QT_RUNTIME").expect("DX_QT_RUNTIME must name files");
    let core = paths
        .split_whitespace()
        .find(|path| file_name(path) == "Qt6Core.dll")
        .unwrap_or_else(|| panic!("DX_QT_RUNTIME holds no Qt6Core.dll: {paths}"));
    let bin = dx_testing::resolve_runfiles(core)
        .parent()
        .expect("runtime directory")
        .to_str()
        .expect("utf-8 path")
        .to_owned();
    let redist = directory_of("DX_QT_VCRUNTIME", "vcruntime140.dll");
    let redist = dx_testing::resolve_runfiles(&redist)
        .to_str()
        .expect("utf-8 path")
        .to_owned();
    match std::env::var("PATH") {
        Ok(path) => format!("{bin};{redist};{path}"),
        Err(_) => format!("{bin};{redist}"),
    }
}

/// The MSVC runtime must come from the pinned payload, never from the host System32.
#[cfg(windows)]
fn host_msvc_runtime(path: &str) -> bool {
    let path = path.replace('\\', "/").to_ascii_lowercase();
    let name = file_name(&path);
    (name.starts_with("vcruntime140") || name.starts_with("msvcp140"))
        && path.contains("/windows/system32/")
}

#[cfg(not(windows))]
fn host_msvc_runtime(_path: &str) -> bool {
    false
}

#[cfg(not(windows))]
fn runtime_path() -> String {
    std::env::var("PATH").unwrap_or_default()
}

fn directory_of(variable: &str, name: &str) -> String {
    let paths = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must name files"));
    let path = paths
        .split_whitespace()
        .find(|path| file_name(path) == name)
        .unwrap_or_else(|| panic!("{variable} holds no {name}: {paths}"));
    let (dir, _) = path.rsplit_once('/').expect("directory");
    dir.to_owned()
}

fn run_gui(platform: &str) -> std::io::Result<dx_testing::Run> {
    let fonts = std::env::var("DX_QT_FONTS")
        .unwrap_or_default()
        .split_whitespace()
        .any(|path| file_name(path) == "fonts.conf")
        .then(|| directory_of("DX_QT_FONTS", "fonts.conf"));
    let platforms = directory_of("DX_QT_PLATFORMS", OFFSCREEN);
    #[cfg(windows)]
    let font_dir = directory_of("DX_QT_FONTS", "DejaVuSans.ttf");
    let qmldir = std::env::var("DX_QT_QMLDIR_PATH").expect("DX_QT_QMLDIR_PATH");
    let imports = Path::new(&qmldir)
        .parent()
        .and_then(Path::parent)
        .expect("import root");
    let path = runtime_path();
    let mut envs = vec![
        ("PATH", path.as_str()),
        LOADER_TRACE,
        ("QML_IMPORT_PATH", imports.to_str().expect("utf-8 path")),
        ("QT_QPA_PLATFORM", platform),
        ("QT_QPA_PLATFORM_PLUGIN_PATH", platforms.as_str()),
        ("QT_QUICK_BACKEND", "software"),
    ];
    if let Some(fonts) = fonts.as_deref() {
        envs.push(("FONTCONFIG_PATH", fonts));
    }
    #[cfg(windows)]
    envs.push(("QT_QPA_FONTDIR", font_dir.as_str()));
    envs.extend(&HOST_ROUTES[2..]);
    let (_scratch, gui) = runnable("DX_QT_GUI");
    let mut command = dx_testing::command_for(&gui, &envs)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Qt shows a blocking MessageBox when there is no console window.
        command.creation_flags(0x0000_0010);
    }
    dx_testing::run_command(&mut command, std::time::Duration::from_secs(120))
}

#[test]
fn gui_renders_offscreen_against_only_pinned_libraries() {
    let run = run_gui("offscreen").expect("run gui check");
    assert!(
        run.status.success(),
        "gui check failed:\n{}",
        run.combined()
    );
    assert!(
        run.stdout.contains("Totals: 7 passed, 0 failed"),
        "{}",
        run.stdout
    );
    let objects = initialized_objects(&run.stderr);
    assert_eq!(host_leaks(&objects), Vec::<String>::new());
    for name in GUI_LOADS {
        assert!(loaded(&objects, name), "{name} not loaded: {objects:?}");
    }
}

#[test]
fn gui_refuses_a_platform_plugin_it_does_not_pin() {
    let run = run_gui("xcb").expect("run gui check");
    assert!(!run.status.success(), "{}", run.combined());
    assert!(
        run.stderr
            .contains("Could not find the Qt platform plugin \"xcb\""),
        "{}",
        run.stderr
    );
    // Keep in sync with platform_plugins in the cell layouts.
    assert_eq!(
        available_platform_plugins(&run.stderr),
        expected_platform_plugins(),
        "{}",
        run.stderr
    );
}

#[cfg(all(windows, target_arch = "x86_64"))]
#[test]
fn qt_cc_test_without_a_console_exits_on_a_missing_platform_plugin() {
    use std::os::windows::process::CommandExt;
    let platforms = directory_of("DX_QT_PLATFORMS", OFFSCREEN);
    let path = runtime_path();
    let envs = [
        ("PATH", path.as_str()),
        ("QT_QPA_PLATFORM", "xcb"),
        ("QT_QPA_PLATFORM_PLUGIN_PATH", platforms.as_str()),
    ];
    let (_scratch, fixture) = runnable("DX_QT_CONSOLE_FIXTURE");
    let mut command = dx_testing::command_for(&fixture, &envs).expect("command");
    // DETACHED_PROCESS: the child has no console window, as under Bazel.
    command.creation_flags(0x0000_0008);
    let run = dx_testing::run_command(&mut command, std::time::Duration::from_secs(30))
        .expect("run fixture");
    assert!(!run.status.success(), "{}", run.combined());
    assert!(
        run.stderr.contains("Available platform plugins are:"),
        "{}",
        run.stderr
    );
}

#[test]
fn tools_report_the_pinned_version() {
    for (variable, name) in [
        ("DX_MOC", "moc"),
        ("DX_QMLFORMAT", "qmlformat"),
        ("DX_QMLLINT", "qmllint"),
        ("DX_QMLTYPEREGISTRAR", "qmltyperegistrar"),
        ("DX_RCC", "rcc"),
        ("DX_UIC", "uic"),
    ] {
        let run = dx_testing::run(&env_file(variable), &["--version"], &[]).expect("run tool");
        assert!(run.status.success(), "{name} failed:\n{}", run.combined());
        assert_eq!(
            run.stdout.replace("\r\n", "\n"),
            format!("{name} {}\n", version())
        );
    }
}

#[test]
fn uic_generates_the_form_class() {
    let form = dx_testing::env_file("DX_QT_FORM");
    let run = dx_testing::run(
        &env_file("DX_UIC"),
        &[form.to_str().expect("utf-8 path")],
        &[],
    )
    .expect("run uic");
    assert!(run.status.success(), "uic failed:\n{}", run.combined());
    for needle in [
        "class Ui_Dialog",
        "void setupUi(QDialog *Dialog)",
        "#include <QtWidgets/QLabel>",
        "label->setText(QCoreApplication::translate(\"Dialog\", \"Hello\", nullptr));",
    ] {
        assert!(
            run.stdout.contains(needle),
            "{needle} missing:\n{}",
            run.stdout
        );
    }
}

#[test]
fn qmlformat_rewrites_to_the_qt_layout() {
    let (_scratch, file) = scratch_file("format.qml", UNFORMATTED);
    let run = dx_testing::run(
        &env_file("DX_QMLFORMAT"),
        &[file.to_str().expect("utf-8 path")],
        &[],
    )
    .expect("run qmlformat");
    assert!(
        run.status.success(),
        "qmlformat failed:\n{}",
        run.combined()
    );
    assert_eq!(run.stdout.replace("\r\n", "\n"), FORMATTED);
}

#[test]
fn qmllint_reports_clean_files_as_json() {
    let (_scratch, file) = scratch_file("clean.qml", FORMATTED);
    let run = lint(&file, &[]);
    assert_eq!(run.status.code(), Some(0), "{}", run.combined());
    let report: Value = dx_testing::serde_json::from_str(&run.stdout).expect("JSON report");
    assert_eq!(report["revision"], 4);
    assert_eq!(report["files"][0]["success"], true);
    assert_eq!(report["files"][0]["warnings"], Value::Array(Vec::new()));
}

#[test]
fn qmllint_fails_unqualified_access_with_a_located_warning() {
    let (_scratch, file) = scratch_file("dirty.qml", UNQUALIFIED);
    let run = lint(&file, &[]);
    assert_eq!(
        run.status.code(),
        Some(UNQUALIFIED_EXIT),
        "{}",
        run.combined()
    );
    let report: Value = dx_testing::serde_json::from_str(&run.stdout).expect("JSON report");
    let warning = &report["files"][0]["warnings"][0];
    assert_eq!(report["files"][0]["success"], false);
    assert_eq!(warning["id"], "unqualified");
    assert_eq!(warning["line"], 4);
    assert_eq!(warning["column"], 21);
}

// qmllint is a Qt tool and prints no module list on Windows.
#[cfg(not(windows))]
#[test]
fn tools_ignore_host_routes_and_load_pinned_plugins() {
    let (_scratch, file) = scratch_file("plugins.qml", UNQUALIFIED);
    let mut envs = vec![LOADER_TRACE];
    envs.extend(HOST_ROUTES);
    let run = lint(&file, &envs);
    assert_eq!(run.status.code(), Some(255), "{}", run.stdout);
    let objects = initialized_objects(&run.stderr);
    assert_eq!(host_leaks(&objects), Vec::<String>::new());
    for name in LINT_LOADS {
        assert!(loaded(&objects, name), "{name} not loaded: {objects:?}");
    }
    assert!(
        objects.iter().any(|path| path.ends_with(LINT_PLUGIN)),
        "{objects:?}"
    );
}

#[cfg(target_os = "macos")]
fn expected_licenses() -> BTreeSet<String> {
    BTreeSet::from(["qt".to_owned()])
}

#[cfg(windows)]
fn expected_licenses() -> BTreeSet<String> {
    BTreeSet::from([
        "dejavu-sans-fonts".to_owned(),
        "msvc-redistributable".to_owned(),
        "qt".to_owned(),
    ])
}

#[cfg(target_os = "linux")]
fn expected_licenses() -> BTreeSet<String> {
    [
        "bzip2-libs",
        "dbus-libs",
        "dejavu-sans-fonts",
        "fontconfig",
        "freetype",
        "glib2",
        "graphite2",
        "harfbuzz",
        "icu",
        "libX11",
        "libX11-xcb",
        "libXau",
        "libXext",
        "libbrotli",
        "libcap",
        "libffi",
        "libgcc",
        "libglvnd",
        "libglvnd-egl",
        "libglvnd-glx",
        "libpng",
        "libstdc++",
        "libwayland-client",
        "libwayland-cursor",
        "libwayland-egl",
        "libxcb",
        "libxkbcommon",
        "libxkbcommon-x11",
        "libxml2",
        "libzstd",
        "pcre2",
        "qt",
        "systemd-libs",
        "xcb-util",
        "xcb-util-cursor",
        "xcb-util-image",
        "xcb-util-keysyms",
        "xcb-util-renderutil",
        "xcb-util-wm",
        "xz-libs",
    ]
    .into_iter()
    .chain(CELL_LICENSES.iter().copied())
    .map(str::to_owned)
    .collect()
}

#[test]
fn licenses_cover_qt_and_every_runtime_package() {
    let paths = std::env::var("DX_QT_LICENSES").expect("DX_QT_LICENSES");
    let mut owners = BTreeSet::new();
    for rel in paths.split_whitespace() {
        let (_, tail) = rel.split_once("/licenses/").expect("license path");
        let owner = tail.split('/').next().expect("license owner");
        owners.insert(owner.to_owned());
        let text = std::fs::read(dx_testing::resolve_runfiles(rel)).expect("license text");
        assert!(!text.is_empty(), "{rel} is empty");
    }
    assert_eq!(owners, expected_licenses());
}

#[test]
fn sbom_declares_the_pinned_licenses() {
    let paths = std::env::var("DX_QT_SBOM").expect("DX_QT_SBOM");
    let mut documents = std::collections::BTreeMap::new();
    for rel in paths.split_whitespace() {
        let document = dx_testing::read_json(&dx_testing::resolve_runfiles(rel)).expect("SBOM");
        documents.insert(file_name(rel).to_owned(), document);
    }
    let base = &documents[&format!("qtbase-{}.spdx.json", version())];
    let declarative = &documents[&format!("qtdeclarative-{}.spdx.json", version())];
    let tools = &documents[&format!("qttools-{}.spdx.json", version())];
    for (document, name, license) in [
        (base, "Core", LIBRARY_LICENSE),
        (declarative, "Qml", LIBRARY_LICENSE),
        (declarative, "qmlformat", TOOL_LICENSE),
        (declarative, "qmllint", TOOL_LICENSE),
        (tools, "lrelease", TOOL_LICENSE),
    ] {
        let package = spdx_package(document, name);
        assert_eq!(package["versionInfo"], version(), "{name}");
        assert_eq!(package["licenseDeclared"], license, "{name}");
    }
}

fn run_fresh_copy(copy: &Path) -> std::process::Output {
    for _ in 0..100 {
        match std::process::Command::new(copy)
            .arg("--version")
            .env_remove("RUNFILES_DIR")
            .env_remove("TEST_SRCDIR")
            .env_remove("RUNFILES_MANIFEST_FILE")
            .output()
        {
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            result => return result.expect("run launcher copy"),
        }
    }
    panic!("{copy:?} stayed busy: a concurrent fork kept its write handle open");
}

#[cfg(not(windows))]
fn assert_launcher_refusal(code: Option<i32>, stderr: &str) {
    assert_eq!(code, Some(127));
    assert!(
        stderr.starts_with("rules_dx: cannot find the Qt runfiles of ")
            && stderr.ends_with(" for bin/qmllint\n"),
        "{stderr}"
    );
}

#[cfg(windows)]
fn assert_launcher_refusal(code: Option<i32>, stderr: &str) {
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.starts_with("exec_launcher: no runfiles beside "),
        "{stderr}"
    );
}

#[test]
fn launcher_fails_explicitly_without_the_pinned_runfiles() {
    let launcher = std::fs::canonicalize(env_file("DX_QMLLINT")).expect("launcher");
    let scratch = dx_test_scratch::scratch("qt-launcher");
    let copy = scratch.path().join("qmllint");
    std::fs::copy(&launcher, &copy).expect("copy launcher");
    let output = run_fresh_copy(&copy);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_launcher_refusal(output.status.code(), &stderr);
}

#[cfg(target_os = "macos")]
fn pinned_needs(table: &str) -> std::collections::BTreeMap<String, BTreeSet<String>> {
    let text = std::fs::read_to_string(dx_testing::env_file("DX_QT_NEEDS")).expect("needs tables");
    let tables: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, BTreeSet<String>>,
    > = dx_testing::serde_json::from_str(&text).expect("needs JSON");
    tables.get(table).cloned().expect("table")
}

#[cfg(target_os = "macos")]
fn dylib_needed(path: &Path, frameworks: &BTreeSet<String>) -> BTreeSet<String> {
    let output = std::process::Command::new("/usr/bin/otool")
        .arg("-L")
        .arg(path)
        .output()
        .expect("run otool");
    assert!(output.status.success(), "{path:?}: {output:?}");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .filter_map(|line| {
            let install = line.trim().split(" (").next()?;
            let name = install
                .split('/')
                .find(|part| part.ends_with(".framework"))?;
            frameworks.contains(name).then(|| name.to_owned())
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn dylibs(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            dylibs(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "dylib") {
            found.push(path);
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn the_pinned_dependency_tables_match_the_shipped_frameworks() {
    let qmldir = std::fs::canonicalize(dx_testing::env_file("DX_QT_QMLDIR_PATH")).expect("qmldir");
    let cell = qmldir.ancestors().nth(3).expect("cell root").to_path_buf();
    let mut libraries = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(cell.join("lib")).expect("lib") {
        let path = entry.expect("entry").path();
        let name = file_name(&path.to_string_lossy()).to_owned();
        if let Some(stem) = name.strip_suffix(".framework") {
            libraries.insert(name.clone(), path.join("Versions/A").join(stem));
        }
    }
    let frameworks: BTreeSet<String> = libraries.keys().cloned().collect();
    let table =
        |paths: Vec<(String, PathBuf)>| -> std::collections::BTreeMap<String, BTreeSet<String>> {
            paths
                .into_iter()
                .map(|(name, path)| {
                    let mut needed = dylib_needed(&path, &frameworks);
                    needed.remove(&name);
                    (name, needed)
                })
                .collect()
        };
    let library = table(libraries.into_iter().collect());

    let mut files = Vec::new();
    dylibs(&cell.join("qml"), &mut files);
    let plugin = table(
        files
            .into_iter()
            .map(|path| {
                let relative = path.strip_prefix(cell.join("qml")).expect("below qml");
                (relative.to_string_lossy().into_owned(), path)
            })
            .collect(),
    );

    let runtime_plugin = table(
        [
            "imageformats/libqgif.dylib",
            "imageformats/libqico.dylib",
            "imageformats/libqjpeg.dylib",
            "platforms/libqminimal.dylib",
            "platforms/libqoffscreen.dylib",
            "tls/libqcertonlybackend.dylib",
            "tls/libqopensslbackend.dylib",
            "tls/libqsecuretransportbackend.dylib",
        ]
        .into_iter()
        .map(|name| (name.to_owned(), cell.join("plugins").join(name)))
        .collect(),
    );
    assert_eq!(pinned_needs("library"), library);
    assert_eq!(pinned_needs("plugin"), plugin);
    assert_eq!(pinned_needs("runtime_plugin"), runtime_plugin);
}
