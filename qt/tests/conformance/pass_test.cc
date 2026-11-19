#include "pass_test.h"

#include <QTest>

#include "greet.h"

void PassTest::greetsWorld() {
  QCOMPARE(greet(QStringLiteral("world")), QStringLiteral("hello world"));
}

QTEST_APPLESS_MAIN(PassTest)
