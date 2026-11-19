#pragma once

#include <QObject>

class FailTest : public QObject {
  Q_OBJECT

 private slots:
  void failingCase();
};
