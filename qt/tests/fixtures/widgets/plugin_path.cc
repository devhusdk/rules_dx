#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  const QString path = qEnvironmentVariable("QT_PLUGIN_PATH");
  if (path.isEmpty()) {
    return 1;
  }
  return QFileInfo(QDir(path).filePath("imageformats")).isDir() ? 0 : 2;
}
