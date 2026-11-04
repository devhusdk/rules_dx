#include "greet.h"

std::string dx_greet_static(const std::string &name) {
  return "hello " + name + dx_greet_static_suffix();
}
