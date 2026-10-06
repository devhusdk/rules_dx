// Include- and macro-dependent C probe for the compilation-context fixtures.
#include "cc/tests/fixtures/compilation_db/generated/limits.h"

int classify(int value) {
  if (value < 0) {
    return -1;
  }
#if DX_ENABLE_LEGACY_BRANCH
  else if (value == 0) {
    return DX_ZERO_RESULT;
  }
#endif
  return value;
}