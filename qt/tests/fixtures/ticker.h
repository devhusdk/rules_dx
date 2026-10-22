#pragma once

#include <QObject>

class Ticker : public QObject {
  Q_OBJECT

public:
  int ticks() const { return ticks_; }

public slots:
  void tick() { ++ticks_; }

private:
  int ticks_ = 0;
};
