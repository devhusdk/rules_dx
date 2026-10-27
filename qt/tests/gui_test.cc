#include "qt/tests/gui_test.h"

#include <QApplication>
#include <QColor>
#include <QDialog>
#include <QFile>
#include <QFont>
#include <QFontDatabase>
#include <QFontInfo>
#include <QGuiApplication>
#include <QImage>
#include <QLabel>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <QSet>
#include <QStringList>
#include <QTest>

#include <algorithm>
#include <memory>

#if defined(__APPLE__)
#include <mach-o/dyld.h>
#endif

#include "ui_dialog.h"

namespace {

#if defined(__APPLE__)
const QStringList kHostPrefixes = {"/usr/local/", "/opt/",
                                   "/Library/Frameworks/"};
#else
const QStringList kHostPrefixes = {"/usr/", "/lib/", "/lib64/", "/opt/"};
#endif

const QStringList kHostRuntime = {
    "ld-linux-aarch64.so.1",
    "ld-linux-x86-64.so.2",
    "libc.so.6",
    "libdl.so.2",
    "libm.so.6",
    "libpthread.so.0",
    "libresolv.so.2",
    "librt.so.1",
    "libstdc++.so.6",
};

#if defined(__APPLE__)
QStringList LoadedImages() {
  QStringList paths;
  for (uint32_t index = 0; index < _dyld_image_count(); ++index) {
    paths.append(QString::fromLocal8Bit(_dyld_get_image_name(index)));
  }
  return paths;
}

QSet<QString> MappedObjects() {
  const QStringList images = LoadedImages();
  return QSet<QString>(images.begin(), images.end());
}

QString Soname(const QString &path) {
  return path.section(QLatin1Char('/'), -1);
}

bool IsQtFramework(const QString &path) {
  return path.contains(QStringLiteral(".framework/Versions/A/Qt"));
}

QString PinnedFont() { return QFontInfo(QFont()).family(); }
#else
QSet<QString> MappedObjects() {
  QFile maps(QStringLiteral("/proc/self/maps"));
  if (!maps.open(QIODevice::ReadOnly)) {
    return {};
  }
  QSet<QString> paths;
  for (const QByteArray &line : maps.readAll().split('\n')) {
    const qsizetype slash = line.indexOf('/');
    if (slash < 0) {
      continue;
    }
    const QString path = QString::fromLocal8Bit(line.mid(slash));
    if (path.contains(QStringLiteral(".so"))) {
      paths.insert(path);
    }
  }
  return paths;
}

QString Soname(const QString &path) {
  const QString name = path.section(QLatin1Char('/'), -1);
  const qsizetype so = name.indexOf(QStringLiteral(".so"));
  const qsizetype version = name.indexOf(QLatin1Char('.'), so + 4);
  return version < 0 ? name : name.left(version);
}

QString PinnedFont() { return QStringLiteral("DejaVu Sans"); }
#endif

} // namespace

void GuiTest::runsOnTheOffscreenPlatform() {
  QCOMPARE(QGuiApplication::platformName(), QStringLiteral("offscreen"));
}

void GuiTest::seesOnlyThePinnedFonts() {
#if defined(__APPLE__)
  QVERIFY(!PinnedFont().isEmpty());
  QVERIFY2(QFontDatabase::families().contains(PinnedFont()),
           qPrintable(PinnedFont()));
#else
  const QStringList generic = {QStringLiteral("Monospace"),
                               QStringLiteral("Sans Serif"),
                               QStringLiteral("Serif")};
  QCOMPARE(QFontDatabase::families(),
           QStringList{QStringLiteral("DejaVu Sans")} + generic);
  for (const QString &family : generic) {
    QCOMPARE(QFontInfo(QFont(family)).family(), QStringLiteral("DejaVu Sans"));
  }
#endif
}

void GuiTest::rendersTheForm() {
  QDialog dialog;
  Ui::Dialog form;
  form.setupUi(&dialog);
  QCOMPARE(form.label->text(), QStringLiteral("Hello"));
  QCOMPARE(QFontInfo(form.label->font()).family(), PinnedFont());
  dialog.adjustSize();
  const QImage image = form.label->grab().toImage();
  QVERIFY(!image.isNull());
  int ink = 0;
  for (int y = 0; y < image.height(); ++y) {
    for (int x = 0; x < image.width(); ++x) {
      ink += qGray(image.pixel(x, y)) < 96 ? 1 : 0;
    }
  }
  QVERIFY2(ink > 20, qPrintable(QStringLiteral("%1 dark pixels").arg(ink)));
}

void GuiTest::rendersQuickInSoftware() {
  QQmlEngine engine;
  QQmlComponent component(&engine);
  component.setData(
      "import QtQuick\n"
      "Window {\n"
      "    width: 40\n"
      "    height: 30\n"
      "    color: \"#ff0000\"\n"
      "    Rectangle { x: 20; width: 20; height: 30; color: \"#0000ff\" }\n"
      "}\n",
      QUrl());
  std::unique_ptr<QObject> root(component.create());
  QVERIFY2(root != nullptr, qPrintable(component.errorString()));
  auto *window = qobject_cast<QQuickWindow *>(root.get());
  QVERIFY(window != nullptr);
  window->show();
  QVERIFY(QTest::qWaitForWindowExposed(window));
  QCOMPARE(window->rendererInterface()->graphicsApi(),
           QSGRendererInterface::Software);
  const QImage image = window->grabWindow();
  QCOMPARE(image.size(), QSize(40, 30));
  QCOMPARE(QColor(image.pixel(5, 15)), QColor(QStringLiteral("#ff0000")));
  QCOMPARE(QColor(image.pixel(35, 15)), QColor(QStringLiteral("#0000ff")));
}

void GuiTest::loadsOnlyPinnedLibraries() {
  QStringList leaks;
  QSet<QString> names;
  for (const QString &path : MappedObjects()) {
    const QString name = Soname(path);
    names.insert(name);
    const bool host = std::any_of(
        kHostPrefixes.begin(), kHostPrefixes.end(),
        [&](const QString &prefix) { return path.startsWith(prefix); });
    if (host && !kHostRuntime.contains(name)) {
      leaks.append(path);
    }
  }
  QVERIFY2(leaks.isEmpty(), qPrintable(leaks.join(QLatin1Char('\n'))));
#if defined(__APPLE__)
  QStringList frameworks;
  for (const QString &path : LoadedImages()) {
    if (IsQtFramework(path)) {
      frameworks.append(Soname(path));
    }
  }
  QStringList twice;
  for (const QString &name : frameworks) {
    if (frameworks.count(name) > 1 && !twice.contains(name)) {
      twice.append(name);
    }
  }
  QVERIFY2(twice.isEmpty(), qPrintable(twice.join(QLatin1Char(' '))));
  for (const char *name :
       {"libqoffscreen.dylib", "QtWidgets", "QtQuick", "QtGui", "QtCore"}) {
    QVERIFY2(names.contains(QLatin1String(name)), name);
  }
#else
  for (const char *name :
       {"libqoffscreen.so", "libQt6Widgets.so.6", "libQt6Quick.so.6",
        "libfontconfig.so.1", "libfreetype.so.6", "libdbus-1.so.3",
        "libGL.so.1", "libxkbcommon.so.0"}) {
    QVERIFY2(names.contains(QLatin1String(name)), name);
  }
#endif
}

int main(int argc, char **argv) {
  QApplication app(argc, argv);
  GuiTest test;
  return QTest::qExec(&test, argc, argv);
}
