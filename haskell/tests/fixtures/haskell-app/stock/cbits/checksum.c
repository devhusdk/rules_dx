#include "checksum.h"

/* Adler-32 over the given bytes. */
uint32_t stock_checksum(const char *data, size_t len) {
  uint32_t a = 1;
  uint32_t b = 0;
  for (size_t i = 0; i < len; i++) {
    a = (a + (unsigned char)data[i]) % 65521;
    b = (b + a) % 65521;
  }
  return (b << 16) | a;
}
