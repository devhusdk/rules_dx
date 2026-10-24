#include "examples/qt-rust/app/ledger_test.h"

#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QTest>

#include <climits>
#include <memory>

#include "examples/qt-rust/model/ledger_model.cxxqt.h"

namespace {

std::unique_ptr<QObject> create(QQmlEngine &engine) {
  QQmlComponent component(&engine, QStringLiteral("Ledger.Ui"), QStringLiteral("Main"));
  std::unique_ptr<QObject> root(component.create());
  if (root == nullptr) {
    qWarning("%s", qPrintable(component.errorString()));
  }
  return root;
}

bool deposit(QObject *root, int amount) {
  bool added = false;
  QMetaObject::invokeMethod(root, "deposit", Q_RETURN_ARG(bool, added), Q_ARG(int, amount));
  return added;
}

}  // namespace

void LedgerTest::rustUpdatesProperties() {
  LedgerModel model;
  QSignalSpy count(&model, &LedgerModel::countChanged);
  QSignalSpy total(&model, &LedgerModel::totalChanged);
  QSignalSpy checksum(&model, &LedgerModel::checksumChanged);
  QVERIFY(model.add(5));
  QVERIFY(model.add(7));
  QCOMPARE(model.getCount(), 2);
  QCOMPARE(model.getTotal(), 12);
  QCOMPARE(model.getChecksum(), 0x004c000d);
  QCOMPARE(model.property("total").toInt(), 12);
  QCOMPARE(count.count(), 2);
  QCOMPARE(total.count(), 2);
  QCOMPARE(checksum.count(), 2);
}

void LedgerTest::rustSignalsRejection() {
  LedgerModel model;
  QSignalSpy rejected(&model, &LedgerModel::rejected);
  QSignalSpy count(&model, &LedgerModel::countChanged);
  QVERIFY(!model.add(0));
  QVERIFY(!model.add(-4));
  QCOMPARE(rejected.count(), 2);
  QCOMPARE(rejected.at(1).first().toInt(), -4);
  QCOMPARE(count.count(), 0);
  QCOMPARE(model.getCount(), 0);
  QCOMPARE(model.getChecksum(), 1);
}

void LedgerTest::rustRejectsOverflow() {
  LedgerModel model;
  QSignalSpy rejected(&model, &LedgerModel::rejected);
  QVERIFY(model.add(INT_MAX));
  QVERIFY(!model.add(1));
  QCOMPARE(rejected.count(), 1);
  QCOMPARE(model.getTotal(), INT_MAX);
  QCOMPARE(model.getCount(), 1);
}

void LedgerTest::qmlDrivesRust() {
  QQmlEngine engine;
  std::unique_ptr<QObject> root = create(engine);
  QVERIFY(root != nullptr);
  auto *model = root->property("ledger").value<LedgerModel *>();
  QVERIFY(model != nullptr);
  QVERIFY(deposit(root.get(), 5));
  QVERIFY(!deposit(root.get(), -2));
  QVERIFY(deposit(root.get(), 7));
  QCOMPARE(model->getTotal(), 12);
  QString summary;
  QVERIFY(QMetaObject::invokeMethod(root.get(), "summary", Q_RETURN_ARG(QString, summary)));
  QCOMPARE(summary, QStringLiteral("2 entries, total 12, adler32 004c000d"));
  QCOMPARE(root->property("rejections").toList(), QVariantList{-2});
}

void LedgerTest::qmlCannotWriteRustProperty() {
  QQmlEngine engine;
  QQmlComponent component(&engine);
  component.setData("import Ledger.Model\nLedgerModel { total: 99 }\n", QUrl());
  QVERIFY(component.isError());
  QVERIFY2(component.errorString().contains(QStringLiteral("read-only property")),
           qPrintable(component.errorString()));
}

QTEST_GUILESS_MAIN(LedgerTest)
