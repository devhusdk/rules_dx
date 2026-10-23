use std::path::{Path, PathBuf};
use std::process::Command;

use dx_testing::nested::{self, Case};
use dx_testing::{resolve_runfiles, serde_json, Run};

const APP: &str = "//examples/qt-tasks/app:tasks";

fn manifest() -> hermetic::Manifest {
    let rel =
        std::env::var("DX_QT_TASKS_MANIFEST").expect("DX_QT_TASKS_MANIFEST names the manifest");
    let text = std::fs::read_to_string(resolve_runfiles(&rel)).expect("manifest is readable");
    let base = std::env::current_dir().expect("runfiles cwd");
    hermetic::parse_manifest(&text, &base).expect("manifest parses")
}

struct Consumer {
    dx: PathBuf,
    repo: String,
    root: PathBuf,
    scratch: PathBuf,
    case: Case,
}

impl Consumer {
    fn new(name: &str, repo: Option<&str>) -> Self {
        let mut manifest = manifest();
        let source = std::fs::canonicalize(&manifest.module_file)
            .expect("rules_dx MODULE.bazel resolves")
            .parent()
            .expect("rules_dx source tree")
            .to_path_buf();
        let case = nested::case(name);
        let scratch = case.root().to_path_buf();
        if let Some(repo) = repo {
            let fragment = read_text(&manifest.module_fragment)
                .replace(&format!("@{}//", manifest.repo_name), &format!("@{repo}//"));
            manifest.module_fragment = scratch.join("module_fragment.bazel");
            write(&manifest.module_fragment, &fragment);
            manifest.repo_name = repo.to_owned();
        }
        let root = scratch.join(&manifest.workspace_dir);
        hermetic::materialize(&manifest, &root, &source, "cc").expect("consumer materializes");
        let dx = std::fs::canonicalize(&manifest.dx).expect("dx resolves");
        Consumer {
            dx,
            repo: manifest.repo_name,
            root,
            scratch,
            case,
        }
    }

    fn dx(&self, command: &str, args: &[&str], rest: &[&str]) -> Run {
        let workspace = self.root.to_string_lossy().into_owned();
        let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
        argv.extend_from_slice(args);
        if !rest.is_empty() {
            argv.push("--");
            argv.extend_from_slice(rest);
        }
        self.case.run(&self.dx, &argv, &[])
    }

    fn tasks(&self, args: &[&str]) -> Run {
        self.dx("run", &[APP], args)
    }

    fn quality(&self, verb: &str, tool: &str, scope: &str) -> Run {
        let repo = &self.repo;
        let tool = format!("--@{repo}//config:tool_{tool}=@dx_qt//:{tool}");
        let policy = format!("--@{repo}//config:workspace=//:policy");
        self.dx(verb, &[scope], &[policy.as_str(), tool.as_str()])
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn edit(&self, rel: &str, from: &str, to: &str) -> String {
        let original = read_text(&self.path(rel));
        assert!(original.contains(from), "{rel} lacks {from:?}");
        write(&self.path(rel), &original.replacen(from, to, 1));
        original
    }
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn rename(from: &Path, to: &Path) {
    std::fs::rename(from, to).unwrap_or_else(|error| panic!("rename {from:?}: {error}"));
}

fn assert_exit(result: &Run, code: i32) {
    assert_eq!(result.status.code(), Some(code), "{}", result.combined());
}

fn assert_has(text: &str, needle: &str) {
    assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
}

fn launch(app: &Path, args: &[&str]) -> Run {
    launch_with(app, args, &[])
}

fn launch_with(app: &Path, args: &[&str], env: &[(&str, &str)]) -> Run {
    let output = Command::new(app)
        .args(args)
        .env_clear()
        .envs(env.iter().copied())
        .current_dir(app.parent().expect("app dir"))
        .output()
        .expect("relocated app launches");
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn machine(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    hermetic::elf_machine(&bytes)
}

fn extract(consumer: &Consumer, name: &str) -> PathBuf {
    let bin = consumer.path("bazel-bin/examples/qt-tasks/app");
    let archive = format!("{name}.tar.gz");
    let tarball = std::fs::read(bin.join(&archive)).expect("tarball built");
    let checksum = read_text(&bin.join(format!("{archive}.sha256")));
    assert_eq!(
        checksum,
        format!("{}  {archive}\n", hermetic::sha256_hex(&tarball))
    );
    let elsewhere = consumer.scratch.join(name);
    std::fs::create_dir_all(&elsewhere).expect("extract dir");
    let copy = elsewhere.join(&archive);
    std::fs::write(&copy, &tarball).expect("tarball copied");
    let extracted = Command::new("tar")
        .arg("-xzf")
        .arg(&copy)
        .arg("-C")
        .arg(&elsewhere)
        .status()
        .expect("tar runs");
    assert!(extracted.success());
    elsewhere
}

fn pixel(image: &[u8], x: usize, y: usize) -> [u8; 3] {
    let header = b"P6\n320 72\n255\n";
    assert_eq!(&image[..header.len()], header, "not a 320x72 PPM");
    let at = header.len() + 3 * (y * 320 + x);
    [image[at], image[at + 1], image[at + 2]]
}

const HOST_RUNTIME: &[&str] = &[
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
];

#[test]
fn the_app_builds_tests_and_persists_through_a_renamed_repository() {
    let consumer = Consumer::new("tasks", None);
    assert_has(
        &read_text(&consumer.path("MODULE.bazel")),
        "repo_name = \"my_dx\"",
    );
    let browser = read_text(&consumer.path("examples/qt-tasks/browser/BUILD.bazel"));
    assert_has(
        &browser,
        "load(\"@my_dx//qt/rules:defs.bzl\", \"qt_qml_module\")",
    );
    assert!(!browser.contains("@rules_dx//"), "{browser}");

    assert_exit(&consumer.dx("build", &["//..."], &[]), 0);
    let tested = consumer.dx("test", &["//..."], &[]);
    assert_exit(&tested, 0);
    assert_has(&tested.combined(), "//examples/qt-tasks/app:browser_test");
    assert_has(
        &tested.combined(),
        "//examples/qt-tasks/app:board_quick_test",
    );

    let qmltypes = read_text(
        &consumer.path("bazel-bin/examples/qt-tasks/store/store_qml/Tasks/Store/store.qmltypes"),
    );
    assert_has(&qmltypes, "name: \"TaskModel\"");
    assert_has(&qmltypes, "exports: [\"Tasks.Store/TaskModel 1.0\"]");
    let qm = std::fs::read(
        consumer.path("bazel-bin/examples/qt-tasks/browser/_qt/browser/i18n/tasks_de.qm"),
    )
    .expect("lrelease wrote tasks_de.qm");
    assert_eq!(&qm[..4], b"\x3c\xb8\x64\x18", "not a .qm file");

    let store = consumer.scratch.join("tasks.json");
    let store_arg = store.to_string_lossy().into_owned();
    let added = consumer.tasks(&["--store", &store_arg, "add", "Draw logo", "logo"]);
    assert_exit(&added, 0);
    assert_has(
        &added.stdout,
        "[ ] Draw logo - image Logo\n1 of 1 tasks open\n",
    );
    assert_exit(
        &consumer.tasks(&["--store", &store_arg, "add", "Cut trailer", "intro"]),
        0,
    );
    let done = consumer.tasks(&["--store", &store_arg, "done", "0"]);
    assert_exit(&done, 0);
    assert_has(
        &done.stdout,
        "[x] Draw logo - image Logo\n1 of 2 tasks open\n",
    );
    for row in ["abc", "1x", "7"] {
        let missing = consumer.tasks(&["--store", &store_arg, "done", row]);
        assert_exit(&missing, 2);
        assert_has(&missing.stderr, &format!("tasks: no task {row}\n"));
    }
    let german = consumer.tasks(&["--lang", "de", "--store", &store_arg, "list"]);
    assert_exit(&german, 0);
    assert_has(
        &german.stdout,
        " 0 [x] Draw logo - Bild Logo\n 1 [ ] Cut trailer - Film Intro clip\n1 von 2 Aufgaben offen\n",
    );

    let saved: serde_json::Value =
        serde_json::from_str(&read_text(&store)).expect("the store is JSON");
    assert_eq!(
        saved,
        serde_json::json!([
            {"title": "Draw logo", "asset": "logo", "done": true},
            {"title": "Cut trailer", "asset": "intro", "done": false},
        ])
    );

    let unknown = consumer.tasks(&["--store", &store_arg, "add", "Print", "poster"]);
    assert_exit(&unknown, 2);
    assert_has(
        &unknown.stderr,
        "tasks: unknown asset poster (known: intro, logo, theme)",
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&read_text(&store)).expect("JSON"),
        saved
    );
    let french = consumer.tasks(&["--lang", "fr", "--store", &store_arg, "list"]);
    assert_exit(&french, 2);
    assert_has(&french.stderr, "tasks: no translation :/i18n/tasks_fr.qm");
    write(&store, "{\"title\": ");
    let corrupt = consumer.tasks(&["--store", &store_arg, "list"]);
    assert_exit(&corrupt, 3);
    assert_has(
        &corrupt.stderr,
        &format!("tasks: {store_arg}: not a task list"),
    );

    let store_build = "examples/qt-tasks/store/BUILD.bazel";
    let original = consumer.edit(store_build, "@my_dx//qt/rules", "@rules_dx//qt/rules");
    let default_name = consumer.dx("build", &["//examples/qt-tasks/store"], &[]);
    assert_ne!(
        default_name.status.code(),
        Some(0),
        "{}",
        default_name.combined()
    );
    assert_has(&default_name.combined(), "@rules_dx");
    write(&consumer.path(store_build), &original);
}

#[test]
fn the_relocated_archive_runs_without_the_source_tree_or_an_installed_qt() {
    let consumer = Consumer::new("tasks", None);
    assert_exit(
        &consumer.dx(
            "build",
            &[
                "//examples/qt-tasks/app:board_release_archive",
                "//examples/qt-tasks/app:board_release_checksum",
                "//examples/qt-tasks/app:tasks_release_archive",
                "//examples/qt-tasks/app:tasks_release_checksum",
            ],
            &[],
        ),
        0,
    );
    let elsewhere = extract(&consumer, "tasks_release");
    let board_dir = extract(&consumer, "board_release");
    std::fs::remove_dir_all(&consumer.root).expect("source tree removed");

    let app = elsewhere.join("tasks");
    let resources = elsewhere.join("resources");
    assert_eq!(
        read_text(&elsewhere.join("IDENTITY.txt")),
        "name: tasks_release\narch: x86_64\nos: linux\nlinkage: dynamic\nmin_runtime: glibc-2.34\n"
    );
    assert_has(&read_text(&elsewhere.join("NOTICE")), "bundles Qt 6.11.3");
    for license in ["qt", "icu", "openssl-libs", "libstdc++", "fmt", "llvm"] {
        let dir = resources.join("licenses").join(license);
        let count = std::fs::read_dir(&dir)
            .unwrap_or_else(|error| panic!("{dir:?}: {error}"))
            .count();
        assert!(count > 0, "{dir:?} is empty");
    }
    let libraries: Vec<String> = std::fs::read_dir(resources.join("lib"))
        .expect("resources/lib")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    for unwanted in [
        "libQt6Gui",
        "libQt6Quick",
        "libQt6Widgets",
        "libQt6XcbQpa",
        "libQt6WaylandClient",
        "libQt6EglFS",
    ] {
        assert!(
            !libraries.iter().any(|name| name.starts_with(unwanted)),
            "{unwanted} is bundled: {libraries:?}"
        );
    }
    for member in [
        "resources/lib/libQt6Core.so.6",
        "resources/lib/libQt6Qml.so.6",
        "resources/lib/libicuuc.so.73",
        "resources/qml/QtQml/qmldir",
        "resources/qml/QtQml/libqmlplugin.so",
        "resources/qml/QML/qmldir",
    ] {
        let path = elsewhere.join(member);
        let meta =
            std::fs::symlink_metadata(&path).unwrap_or_else(|error| panic!("{member}: {error}"));
        assert!(meta.file_type().is_file(), "{member} is not a regular file");
    }
    assert_eq!(machine(&app).as_deref(), Some("x86_64"));
    assert_eq!(
        machine(&resources.join("lib/libQt6Core.so.6")).as_deref(),
        Some("x86_64")
    );
    let image = std::fs::read(&app).expect("app bytes");
    assert!(
        image
            .windows(21)
            .any(|window| window == b"$ORIGIN/resources/lib"),
        "the app has no bundle RUNPATH"
    );

    let store = elsewhere.join("tasks.json").to_string_lossy().into_owned();
    let added = launch(&app, &["--store", &store, "add", "Mix score", "theme"]);
    assert_exit(&added, 0);
    assert_eq!(
        added.stdout,
        "[ ] Mix score - audio Theme song\n1 of 1 tasks open\n"
    );
    let german = launch(&app, &["--lang", "de", "--store", &store, "list"]);
    assert_exit(&german, 0);
    assert_eq!(
        german.stdout,
        " 0 [ ] Mix score - Ton Theme song\n1 von 1 Aufgaben offen\n"
    );

    let plugin = resources.join("qml/QtQml/libqmlplugin.so");
    let parked = elsewhere.join("libqmlplugin.so");
    rename(&plugin, &parked);
    let no_plugin = launch(&app, &["--store", &store, "list"]);
    assert_exit(&no_plugin, 1);
    assert_has(
        &no_plugin.stderr,
        "module \"QtQml\" plugin \"qmlplugin\" not found",
    );
    rename(&parked, &plugin);

    let qmldir = resources.join("qml/QtQml/qmldir");
    let parked = elsewhere.join("qmldir");
    rename(&qmldir, &parked);
    let no_import = launch(&app, &["--store", &store, "list"]);
    assert_exit(&no_import, 1);
    assert_has(&no_import.stderr, "module \"QtQml\" is not installed");
    rename(&parked, &qmldir);

    let core = resources.join("lib/libQt6Core.so.6");
    let parked = elsewhere.join("libQt6Core.so.6");
    rename(&core, &parked);
    std::fs::copy(resources.join("lib/libQt6Network.so.6"), &core).expect("foreign Core");
    let foreign = launch(&app, &["--store", &store, "list"]);
    assert_ne!(foreign.status.code(), Some(0), "{}", foreign.combined());
    assert_has(
        &foreign.stderr,
        "libQt6Core.so.6: version `Qt_6.11' not found",
    );
    rename(&parked, &core);

    rename(&resources.join("qml"), &resources.join("imports"));
    let moved = launch(&app, &["--store", &store, "list"]);
    assert_exit(&moved, 1);
    assert_has(&moved.stderr, "tasks: cannot load Tasks.Browser/Main");
    write(
        &elsewhere.join("qt.conf"),
        "[Paths]\nPrefix=resources\nQmlImports=imports\n",
    );
    let configured = launch(&app, &["--store", &store, "list"]);
    assert_exit(&configured, 0);
    assert_eq!(
        configured.stdout,
        " 0 [ ] Mix score - audio Theme song\n1 of 1 tasks open\n"
    );

    let board = board_dir.join("board");
    let resources = board_dir.join("resources");
    assert_eq!(
        read_text(&board_dir.join("IDENTITY.txt")),
        "name: board_release\narch: x86_64\nos: linux\nlinkage: dynamic\nmin_runtime: glibc-2.34\n"
    );
    assert_has(&read_text(&board_dir.join("NOTICE")), "bundles Qt 6.11.3");
    for member in [
        "resources/lib/libQt6Gui.so.6",
        "resources/lib/libQt6Quick.so.6",
        "resources/lib/libfontconfig.so.1",
        "resources/plugins/platforms/libqoffscreen.so",
        "resources/plugins/platforms/libqminimal.so",
        "resources/qml/QtQuick/qmldir",
        "resources/qml/QtQuick/libqtquick2plugin.so",
    ] {
        let path = board_dir.join(member);
        let meta =
            std::fs::symlink_metadata(&path).unwrap_or_else(|error| panic!("{member}: {error}"));
        assert!(meta.file_type().is_file(), "{member} is not a regular file");
    }
    assert_eq!(
        machine(&resources.join("plugins/platforms/libqoffscreen.so")).as_deref(),
        Some("x86_64")
    );
    for unwanted in ["libQt6XcbQpa", "libQt6WaylandClient", "libQt6EglFS"] {
        assert!(
            !std::fs::read_dir(resources.join("lib"))
                .expect("resources/lib")
                .any(|entry| entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(unwanted)),
            "{unwanted} is bundled"
        );
    }

    let board_store = board_dir.join("tasks.json");
    write(
        &board_store,
        "[{\"title\": \"Draw logo\", \"asset\": \"logo\", \"done\": true},\n {\"title\": \"Cut trailer\", \"asset\": \"intro\", \"done\": false}]\n",
    );
    let board_store = board_store.to_string_lossy().into_owned();
    let rendered = launch_with(
        &board,
        &["--store", &board_store, "board.ppm"],
        &[("LD_DEBUG", "libs")],
    );
    assert_exit(&rendered, 0);
    assert_eq!(rendered.stdout, "board.ppm 320x72 offscreen\n");
    let image = std::fs::read(board_dir.join("board.ppm")).expect("board.ppm written");
    assert_eq!(pixel(&image, 12, 36), [46, 125, 50], "done box");
    assert_eq!(pixel(&image, 12, 60), [198, 40, 40], "open box");
    assert_eq!(pixel(&image, 300, 12), [255, 255, 255], "background");
    let loaded: Vec<&str> = rendered
        .stderr
        .lines()
        .filter_map(|line| line.split("calling init: ").nth(1))
        .collect();
    let root = board_dir.to_string_lossy().into_owned();
    assert!(
        loaded
            .iter()
            .any(|path| path.ends_with("/resources/plugins/platforms/libqoffscreen.so")),
        "{loaded:?}"
    );
    let host: Vec<&&str> = loaded
        .iter()
        .filter(|path| !path.starts_with(root.as_str()))
        .filter(|path| {
            !HOST_RUNTIME
                .iter()
                .any(|name| path.ends_with(&format!("/{name}")))
        })
        .collect();
    assert!(host.is_empty(), "host libraries loaded: {host:?}");

    let xcb = launch_with(
        &board,
        &["--store", &board_store, "xcb.ppm"],
        &[("QT_QPA_PLATFORM", "xcb")],
    );
    assert_ne!(xcb.status.code(), Some(0), "{}", xcb.combined());
    assert_has(&xcb.stderr, "Could not find the Qt platform plugin \"xcb\"");
    assert_has(&xcb.stderr, "Available platform plugins are: ");

    let offscreen = resources.join("plugins/platforms/libqoffscreen.so");
    let parked = board_dir.join("libqoffscreen.so");
    rename(&offscreen, &parked);
    let no_platform = launch(&board, &["--store", &board_store, "none.ppm"]);
    assert_ne!(
        no_platform.status.code(),
        Some(0),
        "{}",
        no_platform.combined()
    );
    assert_has(
        &no_platform.stderr,
        "Could not find the Qt platform plugin \"offscreen\"",
    );
    assert!(!board_dir.join("none.ppm").exists());
    rename(&parked, &offscreen);

    write(&board_dir.join("tasks.json"), "{");
    let corrupt = launch(&board, &["--store", &board_store, "corrupt.ppm"]);
    assert_exit(&corrupt, 3);
    assert_has(
        &corrupt.stderr,
        &format!("board: {board_store}: not a task list"),
    );
    assert!(!board_dir.join("corrupt.ppm").exists());
}

#[test]
fn quality_and_interface_errors_fail_inside_the_modules() {
    let consumer = Consumer::new("tasks", Some("rules_dx"));
    let main_qml = "examples/qt-tasks/browser/Main.qml";

    assert_exit(
        &consumer.quality("lint", "qmllint", "//examples/qt-tasks/..."),
        0,
    );
    assert_exit(
        &consumer.quality("format", "qmlformat", "//examples/qt-tasks/..."),
        0,
    );

    let original = consumer.edit(main_qml, "arg(tasks.remaining)", "arg(tasks.remainder)");
    let lint = consumer.quality("lint", "qmllint", "//examples/qt-tasks/browser");
    assert_exit(&lint, 1);
    assert_has(&lint.stdout, "examples/qt-tasks/browser/Main.qml:");
    assert_has(
        &lint.stdout,
        "Member \"remainder\" not found on type \"TaskModel\"",
    );
    write(&consumer.path(main_qml), &original);

    let store = "examples/qt-tasks/store/BUILD.bazel";
    let original_store = consumer.edit(store, "    qml_depends = [\"QtQml.Models\"],\n", "");
    let depends = consumer.quality("lint", "qmllint", "//examples/qt-tasks/browser");
    assert_exit(&depends, 1);
    assert_has(&depends.stdout, "QAbstractListModel was not found");
    write(&consumer.path(store), &original_store);

    consumer.edit(
        main_qml,
        "    property TaskModel tasks",
        "property  TaskModel tasks",
    );
    let format = consumer.quality("format", "qmlformat", "//examples/qt-tasks/browser");
    assert_exit(&format, 1);
    assert_has(&format.combined(), "examples/qt-tasks/browser/Main.qml");
    write(&consumer.path(main_qml), &original);

    consumer.edit(main_qml, "import Tasks.Store", "import Tasks.Storage");
    let import = consumer.tasks(&["--store", "unused.json", "list"]);
    assert_exit(&import, 1);
    assert_has(&import.stderr, "module \"Tasks.Storage\" is not installed");
    write(&consumer.path(main_qml), &original);

    let header = "examples/qt-tasks/store/task_model.h";
    let original = consumer.edit(header, "READ count NOTIFY", "READ countMissing NOTIFY");
    let generated = consumer.dx("build", &["//examples/qt-tasks/store"], &[]);
    assert_ne!(generated.status.code(), Some(0), "{}", generated.combined());
    assert_has(&generated.combined(), "moc_task_model");
    assert_has(&generated.combined(), "countMissing");
    write(&consumer.path(header), &original);

    let catalog = "examples/qt-tasks/catalog/BUILD.bazel";
    let original = consumer.edit(catalog, "    resources = [\"assets.json\"],\n", "");
    let resource = consumer.tasks(&["--store", "unused.json", "list"]);
    assert_exit(&resource, 2);
    assert_has(
        &resource.stderr,
        "tasks: missing resource :/catalog/assets.json",
    );
    write(&consumer.path(catalog), &original);

    let translations_ts = "examples/qt-tasks/browser/tasks_de.ts";
    let original = consumer.edit(translations_ts, "<message>", "<message");
    let translation = consumer.dx("build", &["//examples/qt-tasks/browser"], &[]);
    assert_ne!(
        translation.status.code(),
        Some(0),
        "{}",
        translation.combined()
    );
    assert_has(&translation.combined(), "QtLrelease");
    assert_has(&translation.combined(), "tasks_de.ts");
    write(&consumer.path(translations_ts), &original);
    assert_exit(&consumer.dx("test", &["//examples/qt-tasks/..."], &[]), 0);
}
