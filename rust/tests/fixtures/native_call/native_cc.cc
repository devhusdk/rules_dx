#include "native_cc.h"

extern "C" int native_cc_cxx_value(void) {
  int values[3] = {1, 2, 3};
  return values[0] + values[1] + values[2];
}
