#pragma once

#include <QObject>
#include <QString>
#include <QtQmlIntegration/qqmlintegration.h>

class Counter : public QObject {
  Q_OBJECT
  QML_ELEMENT
  Q_PROPERTY(int value READ value WRITE setValue NOTIFY valueChanged)
  Q_PROPERTY(int step READ step WRITE setStep NOTIFY stepChanged)

public:
  explicit Counter(QObject *parent = nullptr);

  int value() const;
  void setValue(int value);
  int step() const;
  void setStep(int step);

  Q_INVOKABLE void increment();
  Q_INVOKABLE QString label() const;

signals:
  void valueChanged();
  void stepChanged();

private:
  int value_ = 0;
  int step_ = 1;
};
