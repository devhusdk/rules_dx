use std::path::{Path, PathBuf};

use dx_testing::{mkscratch, resolve_runfiles, run, Run};

const POLICY: &str = r#"load("@rules_dx//quality:policy.bzl", "quality_family", "workspace_policy")

quality_family(
    name = "cc_family",
    family_id = "cc",
    disabled = [
        "audit",
        "format",
        "lint",
        "typecheck",
    ],
)

quality_family(
    name = "qml_family",
    family_id = "qml",
    disabled = [
        "audit",
        "typecheck",
    ],
    format = ["qmlformat"],
    lint = ["qmllint"],
)

workspace_policy(
    name = "policy",
    families = [
        ":cc_family",
        ":qml_family",
    ],
)
"#;

const SHELF_BUILD: &str = r#"load("@rules_dx//qt/rules:defs.bzl", "qt_qml_module")

qt_qml_module(
    name = "shelf",
    srcs = ["stock.cc"],
    moc_hdrs = ["stock.h"],
    qml_srcs = ["Main.qml"],
    qt = "@my_qt",
    resource_prefix = "/shelf",
    resources = ["motd.txt"],
    uri = "Org.Shelf",
    visibility = ["//visibility:public"],
    deps = ["//vendor/gauge"],
)
"#;

const STOCK_H: &str = r#"#pragma once

#include <QObject>
#include <QtQmlIntegration/qqmlintegration.h>

inline constexpr int kUnit = 1;

class Stock : public QObject {
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(int count READ count WRITE setCount NOTIFY countChanged)

public:
  using QObject::QObject;

  int count() const { return count_ * kUnit; }
  void setCount(int count);

signals:
  void countChanged();

private:
  int count_ = 0;
};
"#;

const STOCK_CC: &str = r#"#include "shelf/stock.h"

void Stock::setCount(int count) {
  if (count_ == count) {
    return;
  }
  count_ = count;
  emit countChanged();
}
"#;

const MAIN_QML: &str = r#"import QtQml
import Org.Shelf
import Gauge

QtObject {
    id: root

    property Dial dial: Dial {
        level: root.stock.count
    }
    property Stock stock: Stock {
        count: 3
    }
    property string summary: "stock " + root.stock.count + " " + root.dial.reading
}
"#;

const GAUGE_BUILD: &str = r#"load("@rules_dx//qt/rules:defs.bzl", "qt_qml_module")

qt_qml_module(
    name = "gauge",
    qml_srcs = ["Dial.qml"],
    qt = "@my_qt",
    uri = "Gauge",
    visibility = ["//visibility:public"],
)
"#;

const DIAL_QML: &str = r#"import QtQml

QtObject {
    property int level: 0
    property string reading: "level " + level
    property int unused: vendored
}
"#;

const APP_BUILD: &str = r#"load("@rules_dx//qt/rules:defs.bzl", "qt_cc_binary", "qt_cc_test")

qt_cc_binary(
    name = "shelf_app",
    srcs = ["main.cc"],
    modules = ["qml"],
    qt = "@my_qt",
    deps = ["//shelf"],
)

qt_cc_test(
    name = "stock_test",
    size = "small",
    srcs = ["stock_test.cc"],
    moc_hdrs = ["stock_test.h"],
    modules = [
        "qml",
        "test",
    ],
    qt = "@my_qt",
    deps = ["//shelf"],
)
"#;

const MAIN_CC: &str = r#"#include <QCoreApplication>
#include <QFile>
#include <QQmlApplicationEngine>

#include <cstdio>

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  QFile motd(QStringLiteral(":/shelf/motd.txt"));
  if (!motd.open(QIODevice::ReadOnly)) {
    std::fprintf(stderr, "shelf: no motd\n");
    return 2;
  }
  std::printf("%s", motd.readAll().constData());
  QQmlApplicationEngine engine;
  engine.loadFromModule("Org.Shelf", "Main");
  if (engine.rootObjects().isEmpty()) {
    return 1;
  }
  std::printf("%s\n", qPrintable(engine.rootObjects().first()->property("summary").toString()));
  return 0;
}
"#;

const STOCK_TEST_H: &str = r#"#pragma once

#include <QObject>

class StockTest : public QObject {
  Q_OBJECT

private slots:
  void notifiesOnce();
  void evaluatesTheShelfModule();
};
"#;

const STOCK_TEST_CC: &str = r#"#include "app/stock_test.h"

#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QTest>

#include <memory>

#include "shelf/stock.h"

void StockTest::notifiesOnce() {
  Stock stock;
  QSignalSpy spy(&stock, &Stock::countChanged);
  stock.setCount(4);
  stock.setCount(4);
  QCOMPARE(spy.count(), 1);
  QCOMPARE(stock.count(), 4 * kUnit);
}

void StockTest::evaluatesTheShelfModule() {
  QQmlEngine engine;
  QQmlComponent component(&engine, QStringLiteral("Org.Shelf"), QStringLiteral("Main"));
  std::unique_ptr<QObject> root(component.create());
  QVERIFY2(root != nullptr, qPrintable(component.errorString()));
  QCOMPARE(root->property("summary").toString(), QStringLiteral("stock 3 level 3"));
}

QTEST_GUILESS_MAIN(StockTest)
"#;

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn rules_dx_root() -> PathBuf {
    let fixture = resolve_runfiles("qt/tests/fixtures/ticker.h");
    let real =
        std::fs::canonicalize(&fixture).unwrap_or_else(|error| panic!("{fixture:?}: {error}"));
    real.ancestors()
        .nth(4)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

fn consumer() -> PathBuf {
    let rules = rules_dx_root();
    let root = mkscratch("qt-app-consumer").unwrap_or_else(|error| panic!("scratch: {error}"));
    let version = std::fs::read_to_string(rules.join(".bazelversion"))
        .unwrap_or_else(|error| panic!("bazelversion: {error}"));
    write(&root.join(".bazelversion"), &version);
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_rust\", version = \"0.74.0\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n\nqt_sdk = use_extension(\"@rules_dx//qt/sdk:repos.bzl\", \"qt_sdk\")\nqt_sdk.configure(modules = [\"qtdeclarative\"])\nuse_repo(qt_sdk, my_qt = \"dx_qt\")\n",
            rules.display()
        ),
    );
    write(
        &root.join(".bazelrc"),
        "startup --max_idle_secs=120\ncommon --enable_bzlmod\nbuild:dx_dev --compilation_mode=fastbuild\nbuild --platforms=@rules_dx//qt/platforms:linux_x86_64\nbuild --use_target_platform_for_tests\nbuild --remote_download_outputs=all\n",
    );
    write(&root.join("BUILD.bazel"), POLICY);
    write(&root.join("shelf/BUILD.bazel"), SHELF_BUILD);
    write(&root.join("shelf/stock.h"), STOCK_H);
    write(&root.join("shelf/stock.cc"), STOCK_CC);
    write(&root.join("shelf/Main.qml"), MAIN_QML);
    write(&root.join("shelf/motd.txt"), "Shelf motd\n");
    write(&root.join("vendor/gauge/BUILD.bazel"), GAUGE_BUILD);
    write(&root.join("vendor/gauge/Dial.qml"), DIAL_QML);
    write(&root.join("app/BUILD.bazel"), APP_BUILD);
    write(&root.join("app/main.cc"), MAIN_CC);
    write(&root.join("app/stock_test.h"), STOCK_TEST_H);
    write(&root.join("app/stock_test.cc"), STOCK_TEST_CC);
    root
}

fn dx(root: &Path, command: &str, args: &[&str], bazel: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
    argv.extend_from_slice(args);
    if !bazel.is_empty() {
        argv.push("--");
        argv.extend_from_slice(bazel);
    }
    run(&resolve_runfiles(&rel), &argv, &[]).unwrap_or_else(|error| panic!("run dx: {error}"))
}

fn shelf_app(root: &Path) -> Run {
    dx(root, "run", &["//app:shelf_app"], &[])
}

fn quality(root: &Path, command: &str, scope: &str) -> Run {
    let tool = format!("--@rules_dx//config:tool_{command}=@my_qt//:{command}");
    let verb = if command == "qmllint" {
        "lint"
    } else {
        "format"
    };
    dx(
        root,
        verb,
        &[scope],
        &["--@rules_dx//config:workspace=//:policy", tool.as_str()],
    )
}

#[test]
fn app_builds_tests_and_runs_through_a_renamed_qt_repo() {
    let root = consumer();
    let built = dx(&root, "build", &["//..."], &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());

    let tested = dx(&root, "test", &["//..."], &[]);
    assert_eq!(tested.status.code(), Some(0), "{}", tested.combined());
    assert!(
        tested.combined().contains("//app:stock_test"),
        "{}",
        tested.combined()
    );

    let ran = shelf_app(&root);
    assert_eq!(ran.status.code(), Some(0), "{}", ran.combined());
    assert!(
        ran.stdout.contains("Shelf motd\nstock 3 level 3\n"),
        "{}",
        ran.combined()
    );

    let qrc = root.join("bazel-bin/shelf/_qt/shelf/qrc_shelf_qml.cpp");
    let original = read(&qrc);
    let qmltypes = String::from_utf8(read(
        &root.join("bazel-bin/shelf/shelf_qml/Org/Shelf/shelf.qmltypes"),
    ))
    .expect("utf-8 qmltypes");
    assert!(qmltypes.contains("name: \"Stock\""), "{qmltypes}");
    assert!(
        qmltypes.contains("exports: [\"Org.Shelf/Stock 1.0\"]"),
        "{qmltypes}"
    );

    write(
        &root.join("shelf/stock.h"),
        &STOCK_H.replace("kUnit = 1;", "kUnit = 10;"),
    );
    let header = shelf_app(&root);
    assert_eq!(header.status.code(), Some(0), "{}", header.combined());
    assert!(
        header.stdout.contains("stock 30 level 30\n"),
        "{}",
        header.combined()
    );

    write(
        &root.join("shelf/Main.qml"),
        &MAIN_QML.replace("\"stock \"", "\"units \""),
    );
    let qml = shelf_app(&root);
    assert_eq!(qml.status.code(), Some(0), "{}", qml.combined());
    assert!(
        qml.stdout.contains("units 30 level 30\n"),
        "{}",
        qml.combined()
    );
    assert_ne!(read(&qrc), original);
    let stale = dx(&root, "test", &["//app:stock_test"], &[]);
    assert_ne!(stale.status.code(), Some(0), "{}", stale.combined());

    write(&root.join("shelf/stock.h"), STOCK_H);
    write(&root.join("shelf/Main.qml"), MAIN_QML);
    let restored = shelf_app(&root);
    assert_eq!(restored.status.code(), Some(0), "{}", restored.combined());
    assert!(
        restored.stdout.contains("stock 3 level 3\n"),
        "{}",
        restored.combined()
    );
    assert_eq!(read(&qrc), original);
}

#[test]
fn wrong_modules_resources_and_repositories_fail() {
    let root = consumer();

    write(
        &root.join("shelf/BUILD.bazel"),
        &SHELF_BUILD.replace("    deps = [\"//vendor/gauge\"],\n", ""),
    );
    let module = shelf_app(&root);
    assert_ne!(module.status.code(), Some(0), "{}", module.combined());
    assert!(
        module.stderr.contains("module \"Gauge\" is not installed"),
        "{}",
        module.combined()
    );

    write(
        &root.join("shelf/BUILD.bazel"),
        &SHELF_BUILD.replace("    resources = [\"motd.txt\"],\n", ""),
    );
    let resource = shelf_app(&root);
    assert_eq!(resource.status.code(), Some(2), "{}", resource.combined());
    assert!(
        resource.stderr.contains("shelf: no motd"),
        "{}",
        resource.combined()
    );

    write(
        &root.join("shelf/BUILD.bazel"),
        &SHELF_BUILD.replace("\"motd.txt\"", "\"missing.txt\""),
    );
    let missing = dx(&root, "build", &["//shelf"], &[]);
    assert_ne!(missing.status.code(), Some(0), "{}", missing.combined());
    assert!(
        missing.combined().contains("missing.txt"),
        "{}",
        missing.combined()
    );
    write(&root.join("shelf/BUILD.bazel"), SHELF_BUILD);

    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replacen(
            "    modules = [\"qml\"],\n",
            "    modules = [\"qml\"],\n    qml_imports = False,\n",
            1,
        ),
    );
    let imports = shelf_app(&root);
    assert_ne!(imports.status.code(), Some(0), "{}", imports.combined());
    assert!(
        imports.stderr.contains("module \"QtQml\" is not installed"),
        "{}",
        imports.combined()
    );

    write(&root.join("app/dialog.ui"), "<ui version=\"4.0\"><class>Dialog</class><widget class=\"QDialog\" name=\"Dialog\"/></ui>\n");
    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replacen(
            "    modules = [\"qml\"],\n",
            "    forms = [\"dialog.ui\"],\n    modules = [\"qml\"],\n",
            1,
        ),
    );
    let widgets = dx(&root, "build", &["//app:shelf_app"], &[]);
    assert_ne!(widgets.status.code(), Some(0), "{}", widgets.combined());
    assert!(
        widgets
            .combined()
            .contains("Qt target 'widgets' is unsupported"),
        "{}",
        widgets.combined()
    );

    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replace("    qt = \"@my_qt\",\n", ""),
    );
    let default_repo = dx(&root, "build", &["//app:shelf_app"], &[]);
    assert_ne!(
        default_repo.status.code(),
        Some(0),
        "{}",
        default_repo.combined()
    );
    assert!(
        default_repo.combined().contains("@dx_qt"),
        "{}",
        default_repo.combined()
    );
}

#[test]
fn lint_and_format_own_only_hand_written_qml() {
    let root = consumer();

    let clean = quality(&root, "qmllint", "//shelf");
    assert_eq!(clean.status.code(), Some(0), "{}", clean.combined());

    write(
        &root.join("shelf/Main.qml"),
        &MAIN_QML.replace(
            "root.stock.count + \" \"",
            "root.stock.count + missing + \" \"",
        ),
    );
    let dirty = quality(&root, "qmllint", "//shelf");
    assert_eq!(dirty.status.code(), Some(1), "{}", dirty.combined());
    assert!(
        dirty
            .stdout
            .contains("shelf/Main.qml:14:60: warning: Unqualified access [qmllint/unqualified]"),
        "{}",
        dirty.combined()
    );
    for foreign in ["vendor/gauge", "shelf_qml/", "_qt/"] {
        assert!(!dirty.stdout.contains(foreign), "{}", dirty.combined());
    }

    let vendor = quality(&root, "qmllint", "//vendor/gauge");
    assert_eq!(vendor.status.code(), Some(1), "{}", vendor.combined());
    assert!(
        vendor.stdout.contains(
            "vendor/gauge/Dial.qml:6:26: warning: Unqualified access [qmllint/unqualified]"
        ),
        "{}",
        vendor.combined()
    );

    write(&root.join("shelf/Main.qml"), MAIN_QML);
    let formatted = quality(&root, "qmlformat", "//shelf");
    assert_eq!(formatted.status.code(), Some(0), "{}", formatted.combined());

    write(
        &root.join("shelf/Main.qml"),
        &MAIN_QML.replace("    id: root\n", "    id:root\n"),
    );
    let unformatted = quality(&root, "qmlformat", "//shelf");
    assert_eq!(
        unformatted.status.code(),
        Some(1),
        "{}",
        unformatted.combined()
    );
    assert!(
        unformatted
            .stdout
            .contains("shelf/Main.qml:1:1: warning: file is not formatted [qmlformat]"),
        "{}",
        unformatted.combined()
    );
    for foreign in ["vendor/gauge", "shelf_qml/", "_qt/", "stock.h", "stock.cc"] {
        assert!(
            !unformatted.stdout.contains(foreign),
            "{}",
            unformatted.combined()
        );
    }
}
