#ifndef EXAMPLES_ERLANG_APP_CHECKSUM_C_SRC_FNV1A_H_
#define EXAMPLES_ERLANG_APP_CHECKSUM_C_SRC_FNV1A_H_

#include <stddef.h>
#include <stdint.h>

/* 32-bit FNV-1a over len bytes. */
uint32_t fnv1a_32(const unsigned char *data, size_t len);

#endif
