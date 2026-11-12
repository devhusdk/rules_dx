#include <iostream>

#include "cuda/tests/conformance/greet.h"

int main() {
  if (greet("world") == "goodbye world") {
    return 0;
  }
  std::cerr << "failing_case" << std::endl;
  return 1;
}
