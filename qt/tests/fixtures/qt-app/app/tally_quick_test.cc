#include <QDir>
#include <QFile>
#include <QTemporaryDir>
#include <QtQuickTest/quicktest.h>

// Windows runfiles are symlinks and Qt reads them as empty files, so the
// test runs a copy of the QML file.
int main(int argc, char **argv) {
  QTemporaryDir scratch;
  if (!scratch.isValid()) {
    return 1;
  }
  const QString name = QStringLiteral("tst_tally.qml");
  const QString source =
      QStringLiteral("qt/tests/fixtures/qt-app/app/") + name;
  if (!QFile::copy(source, QDir(scratch.path()).filePath(name))) {
    return 1;
  }
  return quick_test_main(argc, argv, "tally",
                         scratch.path().toUtf8().constData());
}
