// Warning-policy fixture: plain C library source.
#include "cc/tests/fixtures/copts/strict.h"

int DxStrictTotal(int value) {
  if (value < 0) {
    return -value;
  }
  return value * 2;
}