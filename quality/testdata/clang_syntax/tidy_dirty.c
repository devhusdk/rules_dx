#include "quality/testdata/clang_syntax/dep.h"

#ifndef TIDY_DEFINE
#error TIDY_DEFINE missing
#endif

int tidy_dirty_value(int value) {
  if (value) {
    return 1;
  } else {
    return dep_value();
  }
}
