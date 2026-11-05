#include "lib.h"

int covered(int x) {
#ifdef ALT_BUILD
  if (x > 100) {
    return x;
  }
#endif
  return x + 1;
}

int uncovered(int x) {
  return x * 2;
}
