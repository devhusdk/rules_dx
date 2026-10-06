// Warning-policy fixture: plain C test source.
#include <assert.h>

#include "cc/tests/fixtures/copts/strict.h"

int main(void) {
  assert(DxStrictTotal(3) == 6);
  assert(DxStrictTotal(-3) == 3);
  return 0;
}