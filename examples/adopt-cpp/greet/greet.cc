// Foreign C++ greeting implementation: adopted without upstream changes.
#include "examples/adopt-cpp/greet/greet.h"

#include <string>

#include "examples/adopt-cpp/greet/helper.h"

std::string Greet(const std::string &name) {
  return "hello " + name + HelperSuffix();
}
