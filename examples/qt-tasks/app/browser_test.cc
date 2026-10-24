#include "examples/qt-tasks/app/browser_test.h"

#include <QCoreApplication>
#include <QFile>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>
#include <QTranslator>

#include <memory>

#include "examples/qt-tasks/catalog/asset_catalog.h"
#include "examples/qt-tasks/store/task_model.h"

namespace {

std::unique_ptr<QObject> browser(QQmlEngine &engine) {
  QQmlComponent component(&engine, QStringLiteral("Tasks.Browser"),
                          QStringLiteral("Main"));
  std::unique_ptr<QObject> root(component.create());
  if (root == nullptr) {
    qWarning("%s", qPrintable(component.errorString()));
  }
  return root;
}

QString call(QObject *root, const char *method) {
  QString out;
  QMetaObject::invokeMethod(root, method, Q_RETURN_ARG(QString, out));
  return out;
}

QString line(QObject *root, int row) {
  QString out;
  QMetaObject::invokeMethod(root, "line", Q_RETURN_ARG(QString, out),
                            Q_ARG(int, row));
  return out;
}

} // namespace

void BrowserTest::modelSignalsTrackEdits() {
  TaskModel model;
  QSignalSpy count(&model, &TaskModel::countChanged);
  QSignalSpy remaining(&model, &TaskModel::remainingChanged);
  QSignalSpy changed(&model, &TaskModel::dataChanged);
  model.add(QStringLiteral("Cut trailer"), QStringLiteral("intro"));
  model.add(QStringLiteral("Mix score"), QStringLiteral("theme"));
  QVERIFY(model.toggle(0));
  QVERIFY(!model.toggle(5));
  QCOMPARE(model.count(), 2);
  QCOMPARE(model.remaining(), 1);
  QCOMPARE(count.count(), 2);
  QCOMPARE(remaining.count(), 3);
  QCOMPARE(changed.count(), 1);
  QCOMPARE(model.data(model.index(0), TaskModel::DoneRole).toBool(), true);
  QCOMPARE(model.roleNames().value(TaskModel::AssetRole), QByteArray("asset"));
  QVERIFY(model.remove(0));
  QCOMPARE(model.title(0), QStringLiteral("Mix score"));
  QCOMPARE(count.count(), 3);
  QCOMPARE(remaining.count(), 3);
}

void BrowserTest::catalogLoadsItsResource() {
  AssetCatalog catalog;
  QVERIFY(catalog.loaded());
  QCOMPARE(catalog.ids(),
           (QStringList{QStringLiteral("intro"), QStringLiteral("logo"),
                        QStringLiteral("theme")}));
  QCOMPARE(catalog.kind(QStringLiteral("logo")), QStringLiteral("image"));
  QCOMPARE(catalog.name(QStringLiteral("theme")), QStringLiteral("Theme song"));
  QVERIFY(!catalog.contains(QStringLiteral("poster")));
}

void BrowserTest::qmlDrivesTheModel() {
  QQmlEngine engine;
  std::unique_ptr<QObject> root = browser(engine);
  QVERIFY(root != nullptr);
  auto *tasks = root->property("tasks").value<TaskModel *>();
  QVERIFY(tasks != nullptr);
  QSignalSpy inserted(tasks, &QAbstractItemModel::rowsInserted);
  bool ok = false;
  QVERIFY(QMetaObject::invokeMethod(root.get(), "add", Q_RETURN_ARG(bool, ok),
                                    Q_ARG(QString, QStringLiteral("Draw logo")),
                                    Q_ARG(QString, QStringLiteral("logo"))));
  QVERIFY(ok);
  QVERIFY(
      QMetaObject::invokeMethod(root.get(), "add", Q_RETURN_ARG(bool, ok),
                                Q_ARG(QString, QStringLiteral("Print poster")),
                                Q_ARG(QString, QStringLiteral("poster"))));
  QVERIFY(!ok);
  QVERIFY(QMetaObject::invokeMethod(root.get(), "toggle",
                                    Q_RETURN_ARG(bool, ok), Q_ARG(int, 0)));
  QVERIFY(ok);
  QCOMPARE(inserted.count(), 1);
  QCOMPARE(tasks->count(), 1);
  QCOMPARE(tasks->remaining(), 0);
  QCOMPARE(line(root.get(), 0), QStringLiteral("[x] Draw logo - image Logo"));
  QCOMPARE(call(root.get(), "summary"), QStringLiteral("0 of 1 tasks open"));
  auto *history = root->property("history").value<QAbstractItemModel *>();
  QVERIFY(history != nullptr);
  QCOMPARE(history->rowCount(), 3);
  const QHash<int, QByteArray> roles = history->roleNames();
  const int event = roles.key("event");
  QCOMPARE(history->data(history->index(1, 0), event).toString(),
           QStringLiteral("rejected"));
  QCOMPARE(history->data(history->index(2, 0), event).toString(),
           QStringLiteral("done"));
}

void BrowserTest::persistsAcrossInstances() {
  QTemporaryDir dir;
  QVERIFY(dir.isValid());
  const QString path = dir.filePath(QStringLiteral("tasks.json"));
  {
    TaskModel model;
    model.setPath(path);
    QVERIFY(model.load());
    QCOMPARE(model.count(), 0);
    model.add(QStringLiteral("Cut trailer"), QStringLiteral("intro"));
    model.add(QStringLiteral("Draw logo"), QStringLiteral("logo"));
    QVERIFY(model.toggle(1));
    QVERIFY(model.save());
  }
  TaskModel reloaded;
  QSignalSpy count(&reloaded, &TaskModel::countChanged);
  reloaded.setPath(path);
  QVERIFY(reloaded.load());
  QCOMPARE(count.count(), 1);
  QCOMPARE(reloaded.count(), 2);
  QCOMPARE(reloaded.asset(0), QStringLiteral("intro"));
  QVERIFY(!reloaded.isDone(0));
  QVERIFY(reloaded.isDone(1));
}

void BrowserTest::rejectsCorruptStorage() {
  QTemporaryDir dir;
  QVERIFY(dir.isValid());
  const QString path = dir.filePath(QStringLiteral("tasks.json"));
  QFile file(path);
  QVERIFY(file.open(QIODevice::WriteOnly));
  file.write("{\"title\": ");
  file.close();
  TaskModel model;
  QSignalSpy failed(&model, &TaskModel::failed);
  model.setPath(path);
  QVERIFY(!model.load());
  QCOMPARE(failed.count(), 1);
  QCOMPARE(failed.first().first().toString(),
           path + QStringLiteral(": not a task list"));
}

void BrowserTest::reportsUnwritableStorage() {
  QTemporaryDir dir;
  QVERIFY(dir.isValid());
  TaskModel model;
  QSignalSpy failed(&model, &TaskModel::failed);
  model.setPath(dir.filePath(QStringLiteral("missing/tasks.json")));
  model.add(QStringLiteral("Cut trailer"), QStringLiteral("intro"));
  QVERIFY(!model.save());
  QCOMPARE(failed.count(), 1);
}

void BrowserTest::translatesToGerman() {
  QTranslator translator;
  QVERIFY(translator.load(QStringLiteral(":/i18n/tasks_de.qm")));
  QVERIFY(QCoreApplication::installTranslator(&translator));
  QQmlEngine engine;
  std::unique_ptr<QObject> root = browser(engine);
  QVERIFY(root != nullptr);
  bool ok = false;
  QVERIFY(QMetaObject::invokeMethod(root.get(), "add", Q_RETURN_ARG(bool, ok),
                                    Q_ARG(QString, QStringLiteral("Mix score")),
                                    Q_ARG(QString, QStringLiteral("theme"))));
  QVERIFY(ok);
  QCOMPARE(line(root.get(), 0),
           QStringLiteral("[ ] Mix score - Ton Theme song"));
  QCOMPARE(call(root.get(), "summary"),
           QStringLiteral("1 von 1 Aufgaben offen"));
  QVERIFY(QCoreApplication::removeTranslator(&translator));
}

QTEST_GUILESS_MAIN(BrowserTest)
