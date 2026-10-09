#include "native.h"

int32_t native_add(int32_t left, int32_t right) {
  return (int32_t)(left + right + NATIVE_OFFSET);
}

int32_t native_offset(void) {
  return (int32_t)NATIVE_OFFSET;
}
