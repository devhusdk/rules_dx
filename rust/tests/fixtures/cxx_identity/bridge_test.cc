// CXX identity bridge test.
#include "rust/tests/fixtures/cxx_identity/bridge.h"

#include <cassert>

int main() {
  assert(bridge_add(2, 3) == 5);
  return 0;
}
