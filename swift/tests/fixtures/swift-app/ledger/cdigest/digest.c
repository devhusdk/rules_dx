#include "digest.h"

uint32_t ledger_digest(const uint8_t *bytes, size_t count) {
  uint32_t hash = 2166136261u;
  for (size_t i = 0; i < count; i++) {
    hash ^= bytes[i];
    hash *= 16777619u;
  }
  return hash;
}
