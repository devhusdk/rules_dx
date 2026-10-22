#include "examples/qt-app/tally/counter.h"

Counter::Counter(QObject *parent) : QObject(parent) {}

int Counter::value() const { return value_; }

void Counter::setValue(int value) {
  if (value == value_) {
    return;
  }
  value_ = value;
  emit valueChanged();
}

int Counter::step() const { return step_; }

void Counter::setStep(int step) {
  if (step == step_) {
    return;
  }
  step_ = step;
  emit stepChanged();
}

void Counter::increment() { setValue(value_ + step_); }

QString Counter::label() const {
  return QStringLiteral("tally %1").arg(value_);
}
