#pragma once

#include <QObject>

class BrowserTest : public QObject {
  Q_OBJECT

private slots:
  void modelSignalsTrackEdits();
  void catalogLoadsItsResource();
  void qmlDrivesTheModel();
  void persistsAcrossInstances();
  void rejectsCorruptStorage();
  void reportsUnwritableStorage();
  void translatesToGerman();
};
