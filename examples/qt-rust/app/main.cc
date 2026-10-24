#include <QCoreApplication>
#include <QMetaObject>
#include <QObject>
#include <QQmlApplicationEngine>
#include <QString>
#include <QStringList>
#include <QVariant>

#include <cstdio>

namespace {

constexpr int kUsage = 2;
constexpr int kRejected = 3;

int usage() {
  std::fputs("usage: ledger <amount>...\n", stderr);
  return kUsage;
}

void print(const QString &text) {
  std::printf("%s\n", text.toUtf8().constData());
}

} // namespace

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  const QStringList args = QCoreApplication::arguments().mid(1);
  if (args.isEmpty()) {
    return usage();
  }
  QList<int> amounts;
  for (const QString &arg : args) {
    bool numeric = false;
    amounts.append(arg.toInt(&numeric));
    if (!numeric) {
      std::fprintf(stderr, "ledger: not an amount: %s\n",
                   arg.toUtf8().constData());
      return kUsage;
    }
  }
  QQmlApplicationEngine engine;
  engine.loadFromModule("Ledger.Ui", "Main");
  if (engine.rootObjects().isEmpty()) {
    std::fputs("ledger: cannot load Ledger.Ui/Main\n", stderr);
    return 1;
  }
  QObject *root = engine.rootObjects().first();
  for (const int amount : amounts) {
    bool added = false;
    QMetaObject::invokeMethod(root, "deposit", Q_RETURN_ARG(bool, added),
                              Q_ARG(int, amount));
    print(QStringLiteral("%1 %2")
              .arg(added ? QStringLiteral("added") : QStringLiteral("rejected"))
              .arg(amount));
  }
  QString summary;
  QMetaObject::invokeMethod(root, "summary", Q_RETURN_ARG(QString, summary));
  print(summary);
  const QVariantList rejections = root->property("rejections").toList();
  if (!rejections.isEmpty()) {
    QStringList seen;
    for (const QVariant &amount : rejections) {
      seen.append(amount.toString());
    }
    std::fprintf(stderr, "ledger: rejected signal for %s\n",
                 seen.join(QStringLiteral(", ")).toUtf8().constData());
    return kRejected;
  }
  return 0;
}
