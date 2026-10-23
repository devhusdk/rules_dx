#pragma once

#include <QObject>

class PanelTest : public QObject {
  Q_OBJECT

private slots:
  void bumpsTheCounter();
  void rendersTheLabel();
};
