// Native fixture compiled with select-valued copts.
#include "rust/tests/fixtures/native_call/native.h"

#if NATIVE_CALL_OFFSET != 0 && NATIVE_CALL_OFFSET != 100
#error "unsupported NATIVE_CALL_OFFSET value"
#endif

int32_t native_add(int32_t left, int32_t right) {
  return (int32_t)(left + right + NATIVE_CALL_OFFSET);
}

int32_t native_div(int32_t numer, int32_t denom, int32_t *err) {
  if (!err) {
    return 0;
  }
  if (denom == 0) {
    *err = 1;
    return 0;
  }
  *err = 0;
  return numer / denom;
}

int32_t native_offset(void) {
  return (int32_t)NATIVE_CALL_OFFSET;
}
