#include "examples/qt-tasks/app/board_quick_test.h"

#include <QQmlContext>
#include <QQmlEngine>
#include <QtQuickTest/quicktest.h>

void BoardSetup::qmlEngineAvailable(QQmlEngine *engine) {
  engine->rootContext()->setContextProperty(QStringLiteral("scratch"),
                                            scratch_.path());
}

int main(int argc, char **argv) {
  BoardSetup setup;
  return quick_test_main_with_setup(argc, argv, "board",
                                    "examples/qt-tasks/app", &setup);
}
