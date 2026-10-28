#include "quality/testdata/clang_syntax/dep.h"

int dirty_value(void) { return dep_value() + undeclared_name; }
