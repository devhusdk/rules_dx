// Seed C++ greeter library; consumer of cc_library with the C++17 floor.
#include "cc/tests/fixtures/googletest/greeter.h"

#include <optional>
#include <string>

std::string Greet(const std::string &name) { return "hello " + name; }

std::optional<std::string> MaybeGreet(const std::string &name) {
  if (name.empty()) {
    return std::nullopt;
  }
  return Greet(name);
}
