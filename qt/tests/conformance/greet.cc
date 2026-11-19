#include "greet.h"

QString greet(const QString &name) { return QStringLiteral("hello ") + name; }
