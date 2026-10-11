// Strict generation fixture library; consumer of cc_library.
// Quoted self-include resolves to the owning library and leaves no edge;
// angle includes are toolchain-provided and never produce an edge.
#include "cc/tests/fixtures/strict_generation/strict.h"

#include <string>

std::string StrictGreet(const std::string &name) {
  return "strict hello " + name;
}
