#include "native.h"

int32_t consumer_add(int32_t left, int32_t right) {
  return (int32_t)(left + right + CONSUMER_OFFSET);
}

int32_t consumer_offset(void) {
  return (int32_t)CONSUMER_OFFSET;
}
