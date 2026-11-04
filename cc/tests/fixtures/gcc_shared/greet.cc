#include "greet.h"

#include <stdexcept>
#include <string>

const char *dx_greet_compiler(void) {
  static std::string version;
  try {
    throw std::runtime_error(__VERSION__);
  } catch (const std::exception &e) {
    version = e.what();
  }
  return version.c_str();
}
