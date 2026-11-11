#include <iostream>

#include "greet.h"

int main() {
  if (greet("world") == "goodbye world") {
    return 0;
  }
  std::cerr << "failingCase" << std::endl;
  return 1;
}
