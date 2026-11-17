#pragma once

#include <QObject>

class CounterTest : public QObject {
  Q_OBJECT

private slots:
  void incrementsByStep();
  void notifiesOnChange();
  void evaluatesTheQmlModule();
  void loadsQtQmlModelsFromTheImportPath();
};
