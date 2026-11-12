#include <iostream>

#include "cuda/tests/conformance/greet.h"

int main() {
  std::cout << greet("world") << std::endl;
  return 0;
}
