#include "examples/qt-app/app/counter_test.h"

#include <QQmlComponent>
#include <QQmlEngine>
#include <QSignalSpy>
#include <QTest>
#include <QUrl>

#include <memory>

#include "examples/qt-app/tally/counter.h"

void CounterTest::incrementsByStep() {
  Counter counter;
  counter.setStep(5);
  counter.increment();
  counter.increment();
  QCOMPARE(counter.value(), 10);
  QCOMPARE(counter.label(), QStringLiteral("tally 10"));
}

void CounterTest::notifiesOnChange() {
  Counter counter;
  QSignalSpy spy(&counter, &Counter::valueChanged);
  counter.setValue(3);
  counter.setValue(3);
  QCOMPARE(spy.count(), 1);
}

void CounterTest::evaluatesTheQmlModule() {
  QQmlEngine engine;
  QQmlComponent component(&engine, QStringLiteral("Tally"),
                          QStringLiteral("Main"));
  std::unique_ptr<QObject> root(component.create());
  QVERIFY2(root != nullptr, qPrintable(component.errorString()));
  QVERIFY(QMetaObject::invokeMethod(root.get(), "bump"));
  const QObject *badge = root->property("badge").value<QObject *>();
  QCOMPARE(badge->property("text").toString(), QStringLiteral("tally 42"));
}

void CounterTest::loadsQtQmlModelsFromTheImportPath() {
  QQmlEngine engine;
  QQmlComponent component(&engine);
  component.setData(
      "import QtQml\nimport QtQml.Models\nQtObject {\n"
      "    property ListModel model: ListModel { ListElement { n: 6 } }\n"
      "    property int answer: model.get(0).n * 7\n}\n",
      QUrl(QStringLiteral("file:///models.qml")));
  std::unique_ptr<QObject> object(component.create());
  QVERIFY2(object != nullptr, qPrintable(component.errorString()));
  QCOMPARE(object->property("answer").toInt(), 42);
}

QTEST_GUILESS_MAIN(CounterTest)
