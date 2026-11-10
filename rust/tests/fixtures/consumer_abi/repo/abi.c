#include "abi.h"

#include <stdlib.h>

#if !defined(ABI_API_VERSION)
#error "ABI_API_VERSION did not reach the independent compile"
#endif

struct abi_store {
  int32_t capacity;
  int32_t len;
  int32_t values[];
};

int32_t abi_api_version(void) {
  return (int32_t)ABI_API_VERSION;
}

#if defined(__clang__)
#define ABI_COMPILER_STRING "clang " __clang_version__
#elif defined(__GNUC__)
#define ABI_COMPILER_STRING "gcc " __VERSION__
#else
#define ABI_COMPILER_STRING "unknown-compiler"
#endif

#if defined(__x86_64__)
#define ABI_ARCH_STRING "x86_64"
#elif defined(__aarch64__)
#define ABI_ARCH_STRING "aarch64"
#else
#define ABI_ARCH_STRING "unknown-arch"
#endif

#if defined(_LIBCPP_VERSION)
#define ABI_STL_STRING "libc++"
#elif defined(__GLIBCXX__)
#define ABI_STL_STRING "libstdc++"
#elif defined(_MSC_VER)
#define ABI_STL_STRING "msvc-stl"
#elif defined(__GLIBC__)
#define ABI_STR_(x) #x
#define ABI_STR(x) ABI_STR_(x)
#define ABI_STL_STRING "glibc " ABI_STR(__GLIBC__) "." ABI_STR(__GLIBC_MINOR__)
#else
#define ABI_STL_STRING "unknown-stl"
#endif

const char* abi_build_id(void) {
  return ABI_COMPILER_STRING "|" ABI_ARCH_STRING "|" ABI_STL_STRING;
}

int32_t abi_arch_bits(void) {
  return (int32_t)(sizeof(void*) * 8);
}

abi_store_t* abi_create(int32_t api_version, int32_t capacity) {
  if (api_version != (int32_t)ABI_API_VERSION) {
    return (abi_store_t*)0;
  }
  if (capacity <= 0) {
    return (abi_store_t*)0;
  }
  abi_store_t* store =
      (abi_store_t*)malloc(sizeof(abi_store_t) + (size_t)capacity * sizeof(int32_t));
  if (store == (abi_store_t*)0) {
    return (abi_store_t*)0;
  }
  store->capacity = capacity;
  store->len = 0;
  return store;
}

void abi_destroy(abi_store_t* store) {
  free(store);
}

int32_t abi_push(abi_store_t* store, int32_t value) {
  if (store == (abi_store_t*)0) {
    return ABI_ERROR_ARG;
  }
  if (store->len >= store->capacity) {
    return ABI_ERROR_FULL;
  }
  store->values[store->len] = value;
  store->len += 1;
  return ABI_ERROR_OK;
}

int32_t abi_pop(abi_store_t* store, int32_t* out_value) {
  if (store == (abi_store_t*)0 || out_value == (int32_t*)0) {
    return ABI_ERROR_ARG;
  }
  if (store->len <= 0) {
    return ABI_ERROR_EMPTY;
  }
  store->len -= 1;
  *out_value = store->values[store->len];
  return ABI_ERROR_OK;
}

int32_t abi_len(const abi_store_t* store) {
  if (store == (abi_store_t*)0) {
    return -1;
  }
  return store->len;
}
