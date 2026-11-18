#include "fail_test.h"

#include <QTest>

#include "greet.h"

void FailTest::failingCase() {
  QCOMPARE(greet(QStringLiteral("world")), QStringLiteral("goodbye world"));
}

QTEST_APPLESS_MAIN(FailTest)
