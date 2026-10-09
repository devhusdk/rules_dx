"""A consumer-owned native ABI library built outside the consumer graph."""

_BUILD_BAZEL = '''load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_import")
load("@rules_dx//cc/rules:defs.bzl", "cc_library")

exports_files(["abi.h"])

cc_library(
    name = "abi",
    srcs = ["abi.c"],
    hdrs = ["abi.h"],
    copts = ["-std=c17", "-DABI_API_VERSION=1"],
    includes = ["."],
    visibility = ["//visibility:public"],
)

cc_library(
    name = "abi_mismatch",
    srcs = ["abi.c"],
    hdrs = ["abi.h"],
    copts = ["-std=c17", "-DABI_API_VERSION=999"],
    includes = ["."],
    visibility = ["//visibility:public"],
)

cc_binary(
    name = "abi_shared_bin",
    srcs = [
        "abi.c",
        "abi.h",
    ],
    copts = ["-std=c17", "-DABI_API_VERSION=1"],
    includes = ["."],
    linkshared = True,
    visibility = ["//visibility:public"],
)

cc_import(
    name = "abi_shared",
    hdrs = ["abi.h"],
    includes = ["."],
    shared_library = ":abi_shared_bin",
    visibility = ["//visibility:public"],
)
'''

_ABI_H = '''#ifndef CONSUMER_ABI_H_
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
'''

_ABI_C = '''#include "abi.h"

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
'''

def _consumer_abi_repo_impl(ctx):
    """Writes the C ABI library a consumer of rules_dx would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("abi.h", _ABI_H)
    ctx.file("abi.c", _ABI_C)

consumer_abi_repo = repository_rule(
    implementation = _consumer_abi_repo_impl,
)
