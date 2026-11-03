#ifndef ZIG_APP_DIGEST_H
#define ZIG_APP_DIGEST_H

#include <stddef.h>
#include <stdint.h>

uint64_t manifest_digest(const void *data, size_t len);

#endif
