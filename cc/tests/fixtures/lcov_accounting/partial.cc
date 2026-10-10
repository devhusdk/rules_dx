// Partial-coverage fixture library; two functions hot, one cold.
#include "cc/tests/fixtures/lcov_accounting/partial.h"

int PartialAdd(int a, int b) {
  return a + b;
}

int PartialClamp(int value) {
  if (value < 0) {
    return 0;
  }
  return value;
}

int PartialUntested(int value) {
  if (value == 0) {
    return 1;
  }
  return value * 2;
}
