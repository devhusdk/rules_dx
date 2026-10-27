use dx_testing::nested;
use dx_testing::Run;
use qt_board_support::*;

const APP: &str = "//examples/qt-rust/app:ledger";
const MODEL: &str = "examples/qt-rust/model";

fn ledger(consumer: &Consumer, args: &[&str]) -> Run {
    consumer.dx("run", &[APP], args)
}

fn lint(consumer: &Consumer, scope: &str) -> Run {
    consumer.quality("lint", "qmllint", scope)
}

fn expunge(consumer: &Consumer) {
    assert_exit(
        &nested::bazel_in(&consumer.root, &["clean", "--expunge"]),
        0,
    );
}

fn fetched(consumer: &Consumer) -> Vec<String> {
    let execroot = std::fs::canonicalize(consumer.path("bazel-out")).expect("bazel-out resolves");
    let output_base = execroot
        .ancestors()
        .nth(3)
        .expect("output base")
        .to_path_buf();
    let mut names: Vec<String> = std::fs::read_dir(output_base.join("external"))
        .expect("external dir")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| !name.starts_with('@') && !name.ends_with(".marker"))
        .collect();
    names.sort();
    names
}

fn has_repo(fetched: &[String], fragment: &str) -> bool {
    fetched.iter().any(|name| name.contains(fragment))
}

#[test]
fn qml_drives_the_rust_model_through_generated_bindings() {
    let consumer = Consumer::new("qt-rust-app", "DX_QT_RUST_MANIFEST", None);
    expunge(&consumer);
    let model = read_text(&consumer.path(&format!("{MODEL}/BUILD.bazel")));
    assert_has(
        &model,
        "load(\"@my_dx//qt/rules:rust.bzl\", \"qt_rust_library\")",
    );
    assert!(!model.contains("@rules_dx//"), "{model}");

    assert_exit(
        &consumer.dx("test", &["//examples/qt-rust/ledger/..."], &[]),
        0,
    );
    let pure_rust = fetched(&consumer);
    assert!(has_repo(&pure_rust, "shared_tasks_adler2"), "{pure_rust:?}");
    for optional in ["dx_qt", "crates__cxx-qt-", "crates__cxx-1."] {
        assert!(
            !has_repo(&pure_rust, optional),
            "a pure Rust build fetched {optional}: {pure_rust:?}"
        );
    }

    assert_exit(&consumer.dx("build", &["//..."], &[]), 0);
    let tested = consumer.dx("test", &["//..."], &[]);
    assert_exit(&tested, 0);
    assert_has(&tested.combined(), "//examples/qt-rust/app:ledger_test");
    assert_has(&tested.combined(), "//examples/qt-rust/ledger:ledger_test");
    let full = fetched(&consumer);
    for integration in ["dx_qt", "crates__cxx-qt-0.10.0", "crates__cxx-1.0.202"] {
        assert!(has_repo(&full, integration), "{integration}: {full:?}");
    }

    let bin = consumer.path(&format!("bazel-bin/{MODEL}"));
    let header = read_text(&bin.join("ledger_model.cxxqt.h"));
    assert_has(
        &header,
        "class LedgerModel : public QObject, private ::rust::cxxqt1::CxxQtType<LedgerRust>",
    );
    assert_has(
        &header,
        "Q_PROPERTY(int total READ getTotal NOTIFY totalChanged)",
    );
    assert_has(
        &header,
        "Q_PROPERTY(qlonglong checksum READ getChecksum NOTIFY checksumChanged)",
    );
    assert_has(&header, "Q_SIGNAL void rejected(::std::int32_t amount);");
    assert_has(
        &read_text(&bin.join("ledger_model.cxx.cpp")),
        "cxxbridge1$202$LedgerModel$add(*this, amount)",
    );
    let qmltypes = read_text(&bin.join("model_qml/Ledger/Model/model.qmltypes"));
    assert_has(&qmltypes, "exports: [\"Ledger.Model/LedgerModel 1.0\"]");
    assert_has(&qmltypes, "name: \"rejected\"");
    assert!(
        !qmltypes.contains("type: \"::std::int32_t\";"),
        "{qmltypes}"
    );

    let rejected = ledger(&consumer, &["5", "-2", "7"]);
    assert_exit(&rejected, 3);
    assert_has(
        &rejected.stdout,
        "added 5\nrejected -2\nadded 7\n2 entries, total 12, adler32 004c000d\n",
    );
    assert_has(&rejected.stderr, "ledger: rejected signal for -2\n");
    let accepted = ledger(&consumer, &["1", "2", "3"]);
    assert_exit(&accepted, 0);
    assert_has(
        &accepted.stdout,
        "added 1\nadded 2\nadded 3\n3 entries, total 6, adler32 00340007\n",
    );
    let overflow = ledger(&consumer, &["2147483647", "1"]);
    assert_exit(&overflow, 3);
    assert_has(&overflow.stdout, "added 2147483647\nrejected 1\n");
    let usage = ledger(&consumer, &["five"]);
    assert_exit(&usage, 2);
    assert_has(&usage.stderr, "ledger: not an amount: five");

    let lib = format!("{MODEL}/lib.rs");
    let build = format!("{MODEL}/BUILD.bazel");
    let original_lib = consumer.edit(
        &lib,
        "pub mod ledger_model;",
        "#[cfg(not(consumer_choice))]\ncompile_error!(\"consumer rustc flags did not reach the bridge crate\");\n\npub mod ledger_model;",
    );
    let unflagged = consumer.dx("build", &[APP], &[]);
    assert_fails(&unflagged);
    assert_has(
        &unflagged.combined(),
        "consumer rustc flags did not reach the bridge crate",
    );
    let original_build = consumer.edit(
        &build,
        "    crate_root = \"lib.rs\",\n",
        "    crate_root = \"lib.rs\",\n    rustc_flags = [\"--cfg=consumer_choice\"],\n",
    );
    assert_exit(&consumer.dx("build", &[APP], &[]), 0);
    write(&consumer.path(&lib), &original_lib);
    write(&consumer.path(&build), &original_build);

    let module = consumer.edit(
        "MODULE.bazel",
        "qt_sdk.configure(modules = [\"qtdeclarative\"])",
        "qt_sdk.configure(modules = [\"qtbase\"])",
    );
    let no_qml = consumer.dx("build", &["//examples/qt-rust/model"], &[]);
    assert_fails(&no_qml);
    assert_has(
        &no_qml.combined(),
        "enable module 'qtdeclarative' in qt_sdk.configure(modules = ...)",
    );
    write(&consumer.path("MODULE.bazel"), &module);
}

#[test]
fn a_cpp_qml_consumer_does_not_fetch_the_rust_integration() {
    let consumer = Consumer::new("qt-rust-cpp-only", "DX_QT_TASKS_MANIFEST", None);
    expunge(&consumer);
    assert_exit(&consumer.dx("test", &["//..."], &[]), 0);
    let fetched = fetched(&consumer);
    assert!(has_repo(&fetched, "dx_qt"), "{fetched:?}");
    for optional in ["crates__cxx-qt-", "crates__cxx-1.", "crates__cxx-gen-"] {
        assert!(
            !has_repo(&fetched, optional),
            "a C++/QML build fetched {optional}: {fetched:?}"
        );
    }
}

#[test]
fn the_relocated_archive_runs_without_the_source_tree_or_an_installed_qt() {
    let consumer = Consumer::new("qt-rust-archive", "DX_QT_RUST_MANIFEST", None);
    assert_exit(
        &consumer.dx(
            "build",
            &[
                "//examples/qt-rust/app:ledger_release_archive",
                "//examples/qt-rust/app:ledger_release_checksum",
            ],
            &[],
        ),
        0,
    );
    let elsewhere = extract(&consumer, "examples/qt-rust/app", "ledger_release");
    std::fs::remove_dir_all(&consumer.root).expect("source tree removed");

    let app = elsewhere.join("ledger");
    let resources = elsewhere.join("resources");
    assert_eq!(
        read_text(&elsewhere.join("IDENTITY.txt")),
        "name: ledger_release\narch: x86_64\nos: linux\nlinkage: dynamic\nmin_runtime: glibc-2.34\n"
    );
    assert_has(
        &read_text(&elsewhere.join("NOTICE")),
        "the Rust crates cxx, cxx-qt and adler2 under Apache-2.0",
    );
    for license in ["qt", "icu", "apache", "llvm"] {
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
    assert!(
        libraries.iter().any(|name| name == "libQt6Qml.so.6"),
        "{libraries:?}"
    );
    for unwanted in ["libstd-", "libQt6Gui", "libQt6Quick", "libQt6Widgets"] {
        assert!(
            !libraries.iter().any(|name| name.starts_with(unwanted)),
            "{unwanted} is bundled: {libraries:?}"
        );
    }
    assert_eq!(machine(&app).as_deref(), Some("x86_64"));
    let image = std::fs::read(&app).expect("app bytes");
    for symbol in [
        b"cxxbridge1$202$LedgerModel$add".as_slice(),
        b"$ORIGIN/resources/lib".as_slice(),
    ] {
        assert!(
            image.windows(symbol.len()).any(|window| window == symbol),
            "the app lacks {}",
            String::from_utf8_lossy(symbol)
        );
    }

    let ran = launch(&app, &["5", "-2", "7"]);
    assert_exit(&ran, 3);
    assert_eq!(
        ran.stdout,
        "added 5\nrejected -2\nadded 7\n2 entries, total 12, adler32 004c000d\n"
    );
    assert_has(&ran.stderr, "ledger: rejected signal for -2\n");
    assert_exit(&launch(&app, &["4"]), 0);

    let plugin = resources.join("qml/QtQml/libqmlplugin.so");
    let parked = elsewhere.join("libqmlplugin.so");
    rename(&plugin, &parked);
    let no_plugin = launch(&app, &["4"]);
    assert_exit(&no_plugin, 1);
    assert_has(
        &no_plugin.stderr,
        "module \"QtQml\" plugin \"qmlplugin\" not found",
    );
    rename(&parked, &plugin);

    let core = resources.join("lib/libQt6Core.so.6");
    let parked = elsewhere.join("libQt6Core.so.6");
    rename(&core, &parked);
    std::fs::copy(resources.join("lib/libQt6Network.so.6"), &core).expect("foreign Core");
    let foreign = launch(&app, &["4"]);
    assert_fails(&foreign);
    assert_has(
        &foreign.stderr,
        "libQt6Core.so.6: version `Qt_6.11' not found",
    );
    rename(&parked, &core);
    assert_exit(&launch(&app, &["4"]), 0);
}

#[test]
fn binding_signal_and_property_errors_fail_in_the_owning_target() {
    let consumer = Consumer::new("qt-rust-errors", "DX_QT_RUST_MANIFEST", Some("rules_dx"));
    let bridge = format!("{MODEL}/ledger_model.rs");
    let build = format!("{MODEL}/BUILD.bazel");
    let main_qml = "examples/qt-rust/ui/Main.qml";

    assert_exit(&lint(&consumer, "//examples/qt-rust/ui"), 0);
    let original = consumer.edit(main_qml, "ledger.total + ", "ledger.totl + ");
    let lint = lint(&consumer, "//examples/qt-rust/ui");
    assert_exit(&lint, 1);
    assert_has(&lint.stdout, "examples/qt-rust/ui/Main.qml:");
    assert_has(
        &lint.stdout,
        "Member \"totl\" not found on type \"LedgerModel\"",
    );
    write(&consumer.path(main_qml), &original);

    consumer.edit(main_qml, "function onRejected(", "function onRefused(");
    let unheard = ledger(&consumer, &["-2"]);
    assert_exit(&unheard, 0);
    assert_has(
        &unheard.stdout,
        "rejected -2\n0 entries, total 0, adler32 00000001\n",
    );
    assert_has(&unheard.stderr, "onRefused");
    write(&consumer.path(main_qml), &original);

    let original = consumer.edit(
        &bridge,
        "#[qproperty(i32, total, READ, NOTIFY)]",
        "#[qproperty(i32, total, READ, NOTIFY = refreshed)]",
    );
    let notify = consumer.dx("build", &["//examples/qt-rust/model"], &[]);
    assert_fails(&notify);
    assert_has(&notify.combined(), "QtRustBridge");
    assert_has(
        &notify.combined(),
        "examples/qt-rust/model/ledger_model.rs:14:",
    );
    assert_has(
        &notify.combined(),
        "Signal with name `refreshed` not found!",
    );
    write(&consumer.path(&bridge), &original);

    consumer.edit(
        &bridge,
        "fn rejected(self: Pin<&mut LedgerModel>, amount: i32);",
        "fn rejected(self: &LedgerModel, amount: i32);",
    );
    let signal = consumer.dx("build", &["//examples/qt-rust/model"], &[]);
    assert_fails(&signal);
    assert_has(
        &signal.combined(),
        "immutable signals can only be used in `unsafe extern \"C++Qt\"` blocks",
    );
    write(&consumer.path(&bridge), &original);

    consumer.edit(
        &bridge,
        "self.as_mut().rejected(amount);",
        "self.as_mut().refused(amount);",
    );
    let caller = consumer.dx("build", &["//examples/qt-rust/model:model_rust"], &[]);
    assert_fails(&caller);
    assert_has(&caller.combined(), "model_rust");
    assert_has(&caller.combined(), "refused");
    write(&consumer.path(&bridge), &original);

    consumer.edit(&bridge, "#[cxx_qt::bridge]\n", "");
    let missing = consumer.dx("build", &["//examples/qt-rust/model"], &[]);
    assert_fails(&missing);
    assert_has(
        &missing.combined(),
        "examples/qt-rust/model/ledger_model.rs: holds 0 bridge modules; want exactly one",
    );
    write(&consumer.path(&bridge), &original);

    let original_build = consumer.edit(
        &build,
        "    srcs = [\"lib.rs\"],\n    bridges = [\"ledger_model.rs\"],\n",
        "    srcs = [\n        \"ledger_model.rs\",\n        \"lib.rs\",\n    ],\n    bridges = [],\n",
    );
    let unbridged = consumer.dx("build", &["//examples/qt-rust/app:ledger_test"], &[]);
    assert_fails(&unbridged);
    assert_has(
        &unbridged.combined(),
        "//examples/qt-rust/model:model_bridge",
    );
    assert_has(&unbridged.combined(), "attribute must be non empty");
    write(&consumer.path(&build), &original_build);

    write(
        &consumer.path(&format!("{MODEL}/orphan/ledger_model.rs")),
        &original,
    );
    write(
        &consumer.path(&format!("{MODEL}/orphan/main.cc")),
        "#include \"examples/qt-rust/model/orphan/ledger_model.cxxqt.h\"\n\nint main() { return 0; }\n",
    );

    let stale = read_text(&consumer.path(&bridge)).replace(
        "fn add(self: Pin<&mut LedgerModel>",
        "fn deposit(self: Pin<&mut LedgerModel>",
    );
    write(&consumer.path(&format!("{MODEL}/stale_model.rs")), &stale);
    write(
        &consumer.path(&format!("{MODEL}/stale/main.cc")),
        "#include \"examples/qt-rust/model/stale_model.cxxqt.h\"\n\nint main() {\n  LedgerModel model;\n  return model.deposit(1) ? 0 : 1;\n}\n",
    );
    let repo = &consumer.repo;
    write(
        &consumer.path(&build),
        &format!(
            "load(\"@{repo}//qt/rules:defs.bzl\", \"qt_cc_binary\", \"qt_rust_cc_library\")\n{original_build}\nqt_rust_cc_library(\n    name = \"stale\",\n    bridges = [\"stale_model.rs\"],\n    rust = \":model_rust\",\n    uri = \"Ledger.Stale\",\n)\n\nqt_rust_cc_library(\n    name = \"nested\",\n    bridges = [\"orphan/ledger_model.rs\"],\n    rust = \":model_rust\",\n)\n\nqt_cc_binary(\n    name = \"stale_app\",\n    srcs = [\"stale/main.cc\"],\n    deps = [\":stale\"],\n)\n\nqt_cc_binary(\n    name = \"orphan_app\",\n    srcs = [\"orphan/main.cc\"],\n    deps = [\":model\"],\n)\n"
        ),
    );
    let nested = consumer.dx("build", &["//examples/qt-rust/model:nested"], &[]);
    assert_fails(&nested);
    assert_has(
        &nested.combined(),
        "bridge 'orphan/ledger_model.rs' must sit in the package directory",
    );
    let ungenerated = consumer.dx("build", &["//examples/qt-rust/model:orphan_app"], &[]);
    assert_fails(&ungenerated);
    assert_has(
        &ungenerated.combined(),
        "'examples/qt-rust/model/orphan/ledger_model.cxxqt.h' file not found",
    );
    let mismatch = consumer.dx("build", &["//examples/qt-rust/model:stale_app"], &[]);
    assert_fails(&mismatch);
    assert_has(
        &mismatch.combined(),
        "undefined symbol: cxxbridge1$202$LedgerModel$deposit",
    );
    write(&consumer.path(&build), &original_build);
    for extra in ["stale_model.rs", "stale", "orphan"] {
        let path = consumer.path(&format!("{MODEL}/{extra}"));
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.unwrap_or_else(|error| panic!("remove {path:?}: {error}"));
    }
    assert_exit(&consumer.dx("test", &["//examples/qt-rust/..."], &[]), 0);
}
