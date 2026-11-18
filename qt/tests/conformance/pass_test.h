#pragma once

#include <QObject>

class PassTest : public QObject {
  Q_OBJECT

 private slots:
  void greetsWorld();
};
