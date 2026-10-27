use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use dx_testing::serde_json::Value;

const VERSION: &str = "6.11.3";

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

#[cfg(target_os = "linux")]
const HOST_PREFIXES: &[&str] = &["/usr/", "/lib/", "/lib64/", "/opt/"];

#[cfg(target_os = "macos")]
const HOST_PREFIXES: &[&str] = &["/usr/local/", "/opt/", "/Library/Frameworks/"];

#[cfg(target_os = "macos")]
const HOST_ROUTES: [(&str, &str); 4] = [
    ("DYLD_FRAMEWORK_PATH", "/opt/homebrew/lib:/usr/local/lib"),
    ("QML_IMPORT_PATH", "/opt/homebrew/share/qt/qml"),
    ("QT_PLUGIN_PATH", "/opt/homebrew/share/qt/plugins"),
    ("QT_CONF_PATH", "/opt/homebrew/bin/qt.conf"),
];

#[cfg(target_os = "linux")]
const LOADER_TRACE: (&str, &str) = ("LD_DEBUG", "files");

#[cfg(target_os = "macos")]
const LOADER_TRACE: (&str, &str) = ("DYLD_PRINT_LIBRARIES", "1");

#[cfg(target_os = "linux")]
const OFFSCREEN: &str = "libqoffscreen.so";

#[cfg(target_os = "macos")]
const OFFSCREEN: &str = "libqoffscreen.dylib";

#[cfg(target_os = "linux")]
const PROBE_LOADS: &[&str] = &[
    "libQt6Qml.so.6",
    "libqmlplugin.so",
    "libicuuc.so.73",
    "libglib-2.0.so.0",
];

#[cfg(target_os = "macos")]
const PROBE_LOADS: &[&str] = &["QtQml", "libqmlplugin.dylib"];

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
    for name in PROBE_LOADS {
        assert!(loaded(&objects, name), "{name} not loaded:\n{}", run.stdout);
    }
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

fn run_gui(platform: &str) -> dx_testing::Run {
    let fonts = std::env::var("DX_QT_FONTS")
        .unwrap_or_default()
        .split_whitespace()
        .any(|path| file_name(path) == "fonts.conf")
        .then(|| directory_of("DX_QT_FONTS", "fonts.conf"));
    let platforms = directory_of("DX_QT_PLATFORMS", OFFSCREEN);
    let qmldir = std::env::var("DX_QT_QMLDIR_PATH").expect("DX_QT_QMLDIR_PATH");
    let imports = Path::new(&qmldir)
        .parent()
        .and_then(Path::parent)
        .expect("import root");
    let mut envs = vec![
        LOADER_TRACE,
        ("QML_IMPORT_PATH", imports.to_str().expect("utf-8 path")),
        ("QT_QPA_PLATFORM", platform),
        ("QT_QPA_PLATFORM_PLUGIN_PATH", platforms.as_str()),
        ("QT_QUICK_BACKEND", "software"),
    ];
    if let Some(fonts) = fonts.as_deref() {
        envs.push(("FONTCONFIG_PATH", fonts));
    }
    envs.extend(&HOST_ROUTES[2..]);
    dx_testing::run(&tool("DX_QT_GUI"), &[], &envs).expect("run gui check")
}

#[test]
fn gui_renders_offscreen_against_only_pinned_libraries() {
    let run = run_gui("offscreen");
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
    let run = run_gui("xcb");
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
        BTreeSet::from(["minimal".to_owned(), "offscreen".to_owned()]),
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
        let run = dx_testing::run(&tool(variable), &["--version"], &[]).expect("run tool");
        assert!(run.status.success(), "{name} failed:\n{}", run.combined());
        assert_eq!(run.stdout, format!("{name} {VERSION}\n"));
    }
}

#[test]
fn uic_generates_the_form_class() {
    let form = dx_testing::resolve_runfiles(
        &std::env::var("DX_QT_FORM").expect("DX_QT_FORM must name a form"),
    );
    let run = dx_testing::run(&tool("DX_UIC"), &[form.to_str().expect("utf-8 path")], &[])
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
    let base = &documents[&format!("qtbase-{VERSION}.spdx.json")];
    let declarative = &documents[&format!("qtdeclarative-{VERSION}.spdx.json")];
    let tools = &documents[&format!("qttools-{VERSION}.spdx.json")];
    for (document, name, license) in [
        (base, "Core", LIBRARY_LICENSE),
        (declarative, "Qml", LIBRARY_LICENSE),
        (declarative, "qmlformat", TOOL_LICENSE),
        (declarative, "qmllint", TOOL_LICENSE),
        (tools, "lrelease", TOOL_LICENSE),
    ] {
        let package = spdx_package(document, name);
        assert_eq!(package["versionInfo"], VERSION, "{name}");
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

#[test]
fn launcher_fails_explicitly_without_the_pinned_runfiles() {
    let launcher = std::fs::canonicalize(tool("DX_QMLLINT")).expect("launcher");
    let copy = dx_testing::mkscratch("qt-launcher")
        .expect("scratch directory")
        .join("qmllint");
    std::fs::copy(&launcher, &copy).expect("copy launcher");
    let output = run_fresh_copy(&copy);
    assert_eq!(output.status.code(), Some(127));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("rules_dx: cannot find the Qt runfiles of ")
            && stderr.ends_with(" for bin/qmllint\n"),
        "{stderr}"
    );
}

#[cfg(target_os = "linux")]
fn elf_needed(path: &Path) -> BTreeSet<String> {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    assert_eq!(&bytes[..4], b"\x7fELF", "{path:?}");
    let u16_at =
        |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().expect("u16")) as usize;
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("u32"));
    let u64_at =
        |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("u64")) as usize;
    let (table, size, count) = (u64_at(0x20), u16_at(0x36), u16_at(0x38));
    let headers: Vec<usize> = (0..count).map(|index| table + index * size).collect();
    let loads: Vec<(usize, usize, usize)> = headers
        .iter()
        .filter(|at| u32_at(**at) == 1)
        .map(|at| (u64_at(at + 16), u64_at(at + 8), u64_at(at + 32)))
        .collect();
    let to_offset = |address: usize| {
        loads
            .iter()
            .find(|(base, _, length)| address >= *base && address < base + length)
            .map(|(base, offset, _)| address - base + offset)
            .expect("mapped address")
    };
    let dynamic = headers
        .iter()
        .find(|at| u32_at(**at) == 2)
        .expect("dynamic segment");
    let (start, length) = (u64_at(dynamic + 8), u64_at(dynamic + 32));
    let mut strings = 0;
    let mut names = Vec::new();
    for entry in (start..start + length).step_by(16) {
        match u64_at(entry) {
            0 => break,
            1 => names.push(u64_at(entry + 8)),
            5 => strings = to_offset(u64_at(entry + 8)),
            _ => {}
        }
    }
    names
        .into_iter()
        .map(|name| {
            let from = strings + name;
            let end = bytes[from..].iter().position(|b| *b == 0).expect("NUL");
            String::from_utf8_lossy(&bytes[from..from + end]).into_owned()
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn pinned_needs(table: &str) -> std::collections::BTreeMap<String, BTreeSet<String>> {
    let text = std::fs::read_to_string(dx_testing::resolve_runfiles(
        &std::env::var("DX_QT_NEEDS").expect("DX_QT_NEEDS"),
    ))
    .expect("needs tables");
    let tables: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, BTreeSet<String>>,
    > = dx_testing::serde_json::from_str(&text).expect("needs JSON");
    tables.get(table).cloned().expect("table")
}

#[cfg(target_os = "linux")]
fn shared_objects(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("directory") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            shared_objects(&path, found);
        } else {
            found.push(path);
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn the_pinned_dependency_tables_match_the_shipped_libraries() {
    let qmldir = std::fs::canonicalize(dx_testing::resolve_runfiles(
        &std::env::var("DX_QT_QMLDIR_PATH").expect("DX_QT_QMLDIR_PATH"),
    ))
    .expect("qmldir");
    let cell = qmldir.ancestors().nth(3).expect("cell root").to_path_buf();
    let mut libraries = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(cell.join("lib")).expect("lib") {
        let path = entry.expect("entry").path();
        let name = file_name(&path.to_string_lossy()).to_owned();
        let version = name.rsplit_once(".so.").map(|(_, version)| version);
        if version.is_some_and(|version| version.chars().all(|c| c.is_ascii_digit())) {
            libraries.insert(name, path);
        }
    }
    let known = |path: &Path| {
        elf_needed(path)
            .into_iter()
            .filter(|dep| libraries.contains_key(dep))
            .collect::<BTreeSet<_>>()
    };
    let want: std::collections::BTreeMap<String, BTreeSet<String>> = libraries
        .iter()
        .map(|(name, path)| (name.clone(), known(path)))
        .collect();
    assert_eq!(pinned_needs("library"), want);

    let mut files = Vec::new();
    shared_objects(&cell.join("qml"), &mut files);
    let plugin_want: std::collections::BTreeMap<String, BTreeSet<String>> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "so"))
        .map(|path| {
            let relative = path.strip_prefix(cell.join("qml")).expect("below qml");
            (relative.to_string_lossy().into_owned(), known(path))
        })
        .collect();
    assert_eq!(pinned_needs("plugin"), plugin_want);

    let runtime_plugin_want: std::collections::BTreeMap<String, BTreeSet<String>> = [
        "imageformats/libqgif.so",
        "imageformats/libqico.so",
        "imageformats/libqjpeg.so",
        "platforms/libqminimal.so",
        "platforms/libqoffscreen.so",
        "platforms/libqwayland.so",
        "platforms/libqxcb.so",
        "platformthemes/libqxdgdesktopportal.so",
        "tls/libqcertonlybackend.so",
        "tls/libqopensslbackend.so",
        "wayland-decoration-client/libbradient.so",
        "wayland-shell-integration/libxdg-shell.so",
        "xcbglintegrations/libqxcb-egl-integration.so",
        "xcbglintegrations/libqxcb-glx-integration.so",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), known(&cell.join("plugins").join(name))))
    .collect();
    assert_eq!(pinned_needs("runtime_plugin"), runtime_plugin_want);
}
