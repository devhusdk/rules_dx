// Foreign C++ greeting implementation: adopted without upstream changes.
#include "cc/tests/fixtures/adopt-cpp/greet/greet.h"

#include <string>

#include "cc/tests/fixtures/adopt-cpp/greet/helper.h"

std::string Greet(const std::string &name) {
  return "hello " + name + HelperSuffix();
}
