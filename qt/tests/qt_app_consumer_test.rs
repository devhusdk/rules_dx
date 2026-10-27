use std::collections::BTreeSet;
use std::path::Path;

use dx_testing::nested::{self, Case};
use dx_testing::{resolve_runfiles, rules_dx_source, Run};

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

qt_cc_test(
    name = "view_test",
    size = "small",
    srcs = ["view_test.cc"],
    forms = ["panel.ui"],
    moc_hdrs = ["view_test.h"],
    modules = [
        "quick",
        "test",
        "widgets",
    ],
    qt = "@my_qt",
    deps = ["//shelf"],
)

qt_cc_test(
    name = "shelf_quick_test",
    size = "small",
    srcs = ["shelf_quick_test.cc"],
    data = ["tst_shelf.qml"],
    modules = ["quick_test"],
    qt = "@my_qt",
    deps = ["//shelf"],
)
"#;

const PANEL_UI: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ui version="4.0">
 <class>Panel</class>
 <widget class="QWidget" name="Panel">
  <layout class="QVBoxLayout" name="layout">
   <item>
    <widget class="QLabel" name="label">
     <property name="text">
      <string>stock 0</string>
     </property>
    </widget>
   </item>
  </layout>
 </widget>
 <resources/>
 <connections/>
</ui>
"#;

const VIEW_TEST_H: &str = r#"#pragma once

#include <QObject>

class ViewTest : public QObject {
  Q_OBJECT

private slots:
  void showsTheForm();
  void rendersTheShelfInQuick();
};
"#;

const VIEW_TEST_CC: &str = r##"#include "app/view_test.h"

#include <QColor>
#include <QImage>
#include <QLabel>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QTest>
#include <QWidget>

#include <memory>

#include "shelf/stock.h"
#include "ui_panel.h"

void ViewTest::showsTheForm() {
  QWidget panel;
  Ui::Panel form;
  form.setupUi(&panel);
  QCOMPARE(form.label->text(), QStringLiteral("stock 0"));
  Stock stock;
  stock.setCount(3);
  form.label->setText(QStringLiteral("stock %1").arg(stock.count()));
  panel.show();
  QVERIFY(QTest::qWaitForWindowExposed(&panel));
  QCOMPARE(form.label->text(), QStringLiteral("stock 3"));
}

void ViewTest::rendersTheShelfInQuick() {
  QQmlEngine engine;
  QQmlComponent component(&engine);
  component.setData("import QtQuick\n"
                    "import Org.Shelf\n"
                    "Window {\n"
                    "    property Stock stock: Stock { count: 3 }\n"
                    "    width: 20\n"
                    "    height: 20\n"
                    "    color: stock.count === 3 ? '#00ff00' : '#ff0000'\n"
                    "}\n",
                    QUrl());
  std::unique_ptr<QObject> root(component.create());
  QVERIFY2(root != nullptr, qPrintable(component.errorString()));
  auto *window = qobject_cast<QQuickWindow *>(root.get());
  QVERIFY(window != nullptr);
  window->show();
  QVERIFY(QTest::qWaitForWindowExposed(window));
  QCOMPARE(QColor(window->grabWindow().pixel(10, 10)), QColor(QStringLiteral("#00ff00")));
}

QTEST_MAIN(ViewTest)
"##;

const SHELF_QUICK_TEST_CC: &str = r#"#include <QtQuickTest/quicktest.h>

int main(int argc, char **argv) {
  return quick_test_main(argc, argv, "shelf", "app");
}
"#;

const TST_SHELF_QML: &str = r#"import QtQuick
import QtTest
import Org.Shelf

Item {
    property Stock stock: Stock {
        count: 2
    }

    TestCase {
        name: "Shelf"

        function test_count() {
            compare(parent.stock.count, 2);
        }
    }
}
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

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("consumer");
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
        "common --enable_bzlmod\nbuild:dx_dev --compilation_mode=fastbuild\nbuild --platforms=@rules_dx//qt/platforms:linux_x86_64\nbuild --use_target_platform_for_tests\nbuild --remote_download_outputs=all\n",
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
    write(&root.join("app/panel.ui"), PANEL_UI);
    write(&root.join("app/view_test.h"), VIEW_TEST_H);
    write(&root.join("app/view_test.cc"), VIEW_TEST_CC);
    write(&root.join("app/shelf_quick_test.cc"), SHELF_QUICK_TEST_CC);
    write(&root.join("app/tst_shelf.qml"), TST_SHELF_QML);
    root
}

fn dx(root: &Case, command: &str, args: &[&str], bazel: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = root.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![command, "--workspace", workspace.as_str()];
    argv.extend_from_slice(args);
    if !bazel.is_empty() {
        argv.push("--");
        argv.extend_from_slice(bazel);
    }
    root.run(&resolve_runfiles(&rel), &argv, &[])
}

fn shelf_app(root: &Case) -> Run {
    dx(root, "run", &["//app:shelf_app"], &[])
}

fn quality(root: &Case, command: &str, scope: &str) -> Run {
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
    for test in [
        "//app:stock_test",
        "//app:view_test",
        "//app:shelf_quick_test",
    ] {
        assert!(tested.combined().contains(test), "{}", tested.combined());
    }

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
    let stale = dx(
        &root,
        "test",
        &["//app:stock_test"],
        &["--test_output=errors"],
    );
    assert_ne!(stale.status.code(), Some(0), "{}", stale.combined());
    assert!(
        stale
            .combined()
            .contains("Compared values are not the same")
            && stale.combined().contains("units 30 level 30"),
        "{}",
        stale.combined()
    );

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

    write(
        &root.join("app/BUILD.bazel"),
        &APP_BUILD.replacen(
            "    moc_hdrs = [\"view_test.h\"],\n",
            "    env = {\"QT_QPA_PLATFORM\": \"xcb\"},\n    moc_hdrs = [\"view_test.h\"],\n",
            1,
        ),
    );
    let platform = dx(
        &root,
        "test",
        &["//app:view_test"],
        &["--test_output=errors"],
    );
    assert_ne!(platform.status.code(), Some(0), "{}", platform.combined());
    assert!(
        platform
            .combined()
            .contains("Could not find the Qt platform plugin \"xcb\""),
        "{}",
        platform.combined()
    );
    // Keep in sync with platform_plugins in qt/sdk/layout_linux.bzl.
    assert_eq!(
        available_platform_plugins(&platform.combined()),
        BTreeSet::from(["minimal".to_owned(), "offscreen".to_owned()]),
        "{}",
        platform.combined()
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
