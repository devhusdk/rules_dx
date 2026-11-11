#include <QCoreApplication>
#include <QString>

int main(int argc, char** argv) {
  QCoreApplication app(argc, argv);
  return QString("hello").isEmpty() ? 1 : 0;
}
