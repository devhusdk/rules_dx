#include "native_cc.h"

#include <string>

extern "C" int native_cc_cxx_value(void) {
  std::string value = "native";
  value += "_cxx";
  return static_cast<int>(value.size());
}
