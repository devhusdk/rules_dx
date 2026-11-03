#pragma once

#include <QObject>

class LedgerTest : public QObject {
  Q_OBJECT

private slots:
  void rustUpdatesProperties();
  void rustSignalsRejection();
  void rustRejectsOverflow();
  void qmlDrivesRust();
  void qmlCannotWriteRustProperty();
};
