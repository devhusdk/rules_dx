#include "digest.h"

#include "xxhash.h"

uint64_t manifest_digest(const void *data, size_t len) {
  return XXH64(data, len, 0);
}
