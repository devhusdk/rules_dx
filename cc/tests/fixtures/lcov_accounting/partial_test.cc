// Partial-coverage fixture test; covers PartialAdd and PartialClamp only.
#include <cassert>

#include "cc/tests/fixtures/lcov_accounting/partial.h"

int main() {
  assert(PartialAdd(2, 3) == 5);
  assert(PartialAdd(-1, 1) == 0);
  assert(PartialClamp(4) == 4);
  assert(PartialClamp(-2) == 0);
  return 0;
}
