#include <QTextStream>

#include "greet.h"

int main() {
  QTextStream out(stdout);
  out << greet(QStringLiteral("world")) << Qt::endl;
  return 0;
}
