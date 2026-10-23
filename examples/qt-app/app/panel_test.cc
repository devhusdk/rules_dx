#include "examples/qt-app/app/panel_test.h"

#include <QImage>
#include <QLabel>
#include <QPushButton>
#include <QTest>
#include <QWidget>

#include "examples/qt-app/tally/counter.h"
#include "ui_panel.h"

void PanelTest::bumpsTheCounter() {
  QWidget panel;
  Ui::Panel form;
  form.setupUi(&panel);
  Counter counter;
  counter.setStep(3);
  QObject::connect(form.bump, &QPushButton::clicked, &counter,
                   &Counter::increment);
  QObject::connect(&counter, &Counter::valueChanged, form.label,
                   [&] { form.label->setText(counter.label()); });
  QTest::mouseClick(form.bump, Qt::LeftButton);
  QTest::mouseClick(form.bump, Qt::LeftButton);
  QCOMPARE(form.label->text(), QStringLiteral("tally 6"));
}

void PanelTest::rendersTheLabel() {
  QWidget panel;
  Ui::Panel form;
  form.setupUi(&panel);
  panel.show();
  QVERIFY(QTest::qWaitForWindowExposed(&panel));
  const QImage image = form.label->grab().toImage();
  int ink = 0;
  for (int y = 0; y < image.height(); ++y) {
    for (int x = 0; x < image.width(); ++x) {
      ink += qGray(image.pixel(x, y)) < 96 ? 1 : 0;
    }
  }
  QVERIFY2(ink > 20, qPrintable(QStringLiteral("%1 dark pixels").arg(ink)));
}

QTEST_MAIN(PanelTest)
