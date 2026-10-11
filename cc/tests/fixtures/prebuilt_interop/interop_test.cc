// Prebuilt interop fixture test; STL plus exceptions plus RTTI plus ownership.
#include <cassert>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#include "cc/tests/fixtures/prebuilt_interop/interop.h"

int main() {
  assert(Join({"a", "b", "c"}) == "a,b,c");
  assert(Join({}) == "");
  auto owned = MakeCircle(2.0);
  assert(owned != nullptr);
  assert(IsCircle(owned.get()));
  assert(Describe(owned.get()) == "circle");
  bool threw = false;
  try {
    (void)Describe(nullptr);
  } catch (const std::invalid_argument &) {
    threw = true;
  }
  assert(threw);
  assert(!IsCircle(nullptr));
  return 0;
}
