#ifndef CONSUMER_ABI_H_
#define CONSUMER_ABI_H_

#include <stddef.h>
#include <stdint.h>

#define ABI_ERROR_OK 0
#define ABI_ERROR_ARG 1
#define ABI_ERROR_FULL 2
#define ABI_ERROR_EMPTY 3

#if defined(__cplusplus)
extern "C" {
#endif

typedef struct abi_store abi_store_t;

int32_t abi_api_version(void);
const char* abi_build_id(void);
int32_t abi_arch_bits(void);
abi_store_t* abi_create(int32_t api_version, int32_t capacity);
void abi_destroy(abi_store_t* store);
int32_t abi_push(abi_store_t* store, int32_t value);
int32_t abi_pop(abi_store_t* store, int32_t* out_value);
int32_t abi_len(const abi_store_t* store);

#if defined(__cplusplus)
}
#endif

#endif
