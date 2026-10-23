#include <QCoreApplication>
#include <QFile>
#include <QFileInfo>
#include <QDeadlineTimer>
#include <QGuiApplication>
#include <QImage>
#include <QObject>
#include <QQmlError>
#include <QQuickItem>
#include <QQuickView>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <QStringList>

#include <cstdio>

#include "examples/qt-tasks/store/task_model.h"

namespace {

constexpr int kUsage = 2;
constexpr int kStorage = 3;
constexpr int kImage = 4;

void error(const QString &text) { std::fprintf(stderr, "board: %s\n", qPrintable(text)); }

void use_bundled_fonts() {
  if (qEnvironmentVariableIsSet("FONTCONFIG_FILE") || qEnvironmentVariableIsSet("FONTCONFIG_PATH")) {
    return;
  }
  const QString exe = QFile::symLinkTarget(QStringLiteral("/proc/self/exe"));
  const QString conf = QFileInfo(exe).absolutePath() + QStringLiteral("/resources/fonts/fonts.conf");
  if (QFileInfo::exists(conf)) {
    qputenv("FONTCONFIG_FILE", conf.toLocal8Bit());
  }
}

}  // namespace

int main(int argc, char **argv) {
  use_bundled_fonts();
  if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) {
    qputenv("QT_QPA_PLATFORM", "offscreen");
  }
  QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
  QGuiApplication app(argc, argv);
  const QStringList args = QCoreApplication::arguments().mid(1);
  if (args.size() != 3 || args.at(0) != QStringLiteral("--store")) {
    std::fprintf(stderr, "usage: board --store <path> <image>\n");
    return kUsage;
  }
  QQuickView view;
  view.setResizeMode(QQuickView::SizeViewToRootObject);
  view.loadFromModule("Tasks.Board", "Board");
  if (view.status() != QQuickView::Ready) {
    for (const QQmlError &failure : view.errors()) {
      error(failure.toString());
    }
    error(QStringLiteral("cannot load Tasks.Board/Board"));
    return 1;
  }
  auto *tasks = view.rootObject()->property("tasks").value<TaskModel *>();
  QObject::connect(tasks, &TaskModel::failed, [](const QString &reason) { error(reason); });
  tasks->setPath(args.at(1));
  if (!tasks->load()) {
    return kStorage;
  }
  view.show();
  const QDeadlineTimer deadline(10000);
  while (!view.isExposed() && !deadline.hasExpired()) {
    QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
  }
  const QImage image = view.grabWindow();
  if (image.isNull() || !image.save(args.at(2))) {
    error(QStringLiteral("cannot write ") + args.at(2));
    return kImage;
  }
  std::printf("%s %dx%d %s\n", qPrintable(args.at(2)), image.width(), image.height(),
              qPrintable(QGuiApplication::platformName()));
  return 0;
}
