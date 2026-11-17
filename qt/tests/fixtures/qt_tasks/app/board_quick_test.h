#pragma once

#include <QObject>
#include <QTemporaryDir>

class QQmlEngine;

class BoardSetup : public QObject {
  Q_OBJECT

public slots:
  void qmlEngineAvailable(QQmlEngine *engine);

private:
  QTemporaryDir scratch_;
};
