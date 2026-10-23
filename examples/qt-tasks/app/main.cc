#include <QCoreApplication>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QString>
#include <QStringList>
#include <QTranslator>
#include <QVariant>

#include <fmt/format.h>

#include "examples/qt-tasks/catalog/asset_catalog.h"
#include "examples/qt-tasks/store/task_model.h"

namespace {

constexpr int kUsage = 2;
constexpr int kStorage = 3;

int usage() {
  fmt::print(stderr, "usage: tasks [--lang <code>] --store <path> add <title> <asset> | done <row> | list\n");
  return kUsage;
}

QString line(QObject *root, int row) {
  QString text;
  QMetaObject::invokeMethod(root, "line", Q_RETURN_ARG(QString, text), Q_ARG(int, row));
  return text;
}

void print(const QString &text) { fmt::print("{}\n", text.toUtf8().constData()); }

}  // namespace

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  QStringList args = QCoreApplication::arguments().mid(1);
  QString lang;
  QString store;
  while (args.size() >= 2 && args.first().startsWith(QStringLiteral("--"))) {
    const QString flag = args.takeFirst();
    const QString value = args.takeFirst();
    if (flag == QStringLiteral("--lang")) {
      lang = value;
    } else if (flag == QStringLiteral("--store")) {
      store = value;
    } else {
      return usage();
    }
  }
  if (store.isEmpty() || args.isEmpty()) {
    return usage();
  }
  QTranslator translator;
  if (!lang.isEmpty()) {
    const QString qm = QStringLiteral(":/i18n/tasks_") + lang + QStringLiteral(".qm");
    if (!translator.load(qm)) {
      fmt::print(stderr, "tasks: no translation {}\n", qm.toUtf8().constData());
      return kUsage;
    }
    QCoreApplication::installTranslator(&translator);
  }
  QQmlApplicationEngine engine;
  engine.loadFromModule("Tasks.Browser", "Main");
  if (engine.rootObjects().isEmpty()) {
    fmt::print(stderr, "tasks: cannot load Tasks.Browser/Main\n");
    return 1;
  }
  QObject *root = engine.rootObjects().first();
  const auto *catalog = root->property("catalog").value<AssetCatalog *>();
  auto *tasks = root->property("tasks").value<TaskModel *>();
  if (!catalog->loaded()) {
    fmt::print(stderr, "tasks: missing resource :/catalog/assets.json\n");
    return kUsage;
  }
  QObject::connect(tasks, &TaskModel::failed, [](const QString &reason) {
    fmt::print(stderr, "tasks: {}\n", reason.toUtf8().constData());
  });
  tasks->setPath(store);
  if (!tasks->load()) {
    return kStorage;
  }
  const QString command = args.takeFirst();
  if (command == QStringLiteral("add") && args.size() == 2) {
    bool added = false;
    QMetaObject::invokeMethod(root, "add", Q_RETURN_ARG(bool, added), Q_ARG(QString, args.at(0)),
                              Q_ARG(QString, args.at(1)));
    if (!added) {
      fmt::print(stderr, "tasks: unknown asset {} (known: {})\n", args.at(1).toUtf8().constData(),
                 catalog->ids().join(QStringLiteral(", ")).toUtf8().constData());
      return kUsage;
    }
    if (!tasks->save()) {
      return kStorage;
    }
    print(line(root, tasks->count() - 1));
  } else if (command == QStringLiteral("done") && args.size() == 1) {
    bool toggled = false;
    bool numeric = false;
    const int row = args.at(0).toInt(&numeric);
    if (numeric) {
      QMetaObject::invokeMethod(root, "toggle", Q_RETURN_ARG(bool, toggled), Q_ARG(int, row));
    }
    if (!toggled) {
      fmt::print(stderr, "tasks: no task {}\n", args.at(0).toUtf8().constData());
      return kUsage;
    }
    if (!tasks->save()) {
      return kStorage;
    }
    print(line(root, row));
  } else if (command == QStringLiteral("list") && args.isEmpty()) {
    for (int row = 0; row < tasks->count(); ++row) {
      fmt::print("{:>2} {}\n", row, line(root, row).toUtf8().constData());
    }
  } else {
    return usage();
  }
  QString summary;
  QMetaObject::invokeMethod(root, "summary", Q_RETURN_ARG(QString, summary));
  print(summary);
  return 0;
}
