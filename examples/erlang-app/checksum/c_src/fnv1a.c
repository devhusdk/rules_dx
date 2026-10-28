#include "examples/erlang-app/checksum/c_src/fnv1a.h"

#define FNV_OFFSET_BASIS 2166136261u
#define FNV_PRIME 16777619u

uint32_t fnv1a_32(const unsigned char *data, size_t len) {
  uint32_t hash = FNV_OFFSET_BASIS;
  for (size_t i = 0; i < len; i++) {
    hash ^= data[i];
    hash *= FNV_PRIME;
  }
  return hash;
}
