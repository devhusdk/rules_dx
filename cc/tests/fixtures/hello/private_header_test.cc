// cc_test that keeps a private header in srcs.
#include <cassert>

#include "cc/tests/fixtures/hello/private_helper.h"

int main() {
  assert(Twice(2) == 4);
  assert(Twice(-3) == -6);
  return 0;
}
