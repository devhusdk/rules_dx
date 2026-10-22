#include <QCoreApplication>
#include <QFile>
#include <QQmlApplicationEngine>
#include <QString>
#include <QVariant>

#include <cstdio>

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  QFile banner(QStringLiteral(":/tally/banner.txt"));
  if (!banner.open(QIODevice::ReadOnly)) {
    std::fprintf(stderr, "tally: missing resource :/tally/banner.txt\n");
    return 2;
  }
  std::printf("%s", banner.readAll().constData());
  QQmlApplicationEngine engine;
  engine.loadFromModule("Tally", "Main");
  if (engine.rootObjects().isEmpty()) {
    std::fprintf(stderr, "tally: cannot load Tally/Main\n");
    return 1;
  }
  QObject *root = engine.rootObjects().first();
  const int bumps = argc > 1 ? QString::fromLocal8Bit(argv[1]).toInt() : 1;
  for (int index = 0; index < bumps; ++index) {
    QMetaObject::invokeMethod(root, "bump");
  }
  const QObject *badge = root->property("badge").value<QObject *>();
  std::printf("%s\n",
              badge->property("text").toString().toLocal8Bit().constData());
  return 0;
}
