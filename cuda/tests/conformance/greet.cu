#include "cuda/tests/conformance/greet.h"

std::string greet(const std::string& name) {
  return "hello " + name;
}
