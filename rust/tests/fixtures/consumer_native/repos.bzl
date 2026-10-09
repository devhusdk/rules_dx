"""A consumer-owned native dependency with its own build script."""

_BUILD_BAZEL = '''load("@rules_dx//cc/rules:defs.bzl", "cc_library")

exports_files(["native.h"])

cc_library(
    name = "native",
    srcs = ["native.c"],
    hdrs = ["native.h"],
    copts = ["-DCONSUMER_OFFSET=7"],
    visibility = ["//visibility:public"],
)
'''

_NATIVE_H = '''#ifndef CONSUMER_NATIVE_H_
#define CONSUMER_NATIVE_H_

#include <stdint.h>

#if !defined(CONSUMER_OFFSET)
#error "consumer copts did not reach the native compile"
#endif

int32_t consumer_add(int32_t left, int32_t right);
int32_t consumer_offset(void);

#endif
'''

_NATIVE_C = '''#include "native.h"

int32_t consumer_add(int32_t left, int32_t right) {
  return (int32_t)(left + right + CONSUMER_OFFSET);
}

int32_t consumer_offset(void) {
  return (int32_t)CONSUMER_OFFSET;
}
'''

def _consumer_native_repo_impl(ctx):
    """Writes the C library a consumer of rules_dx would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("native.h", _NATIVE_H)
    ctx.file("native.c", _NATIVE_C)

consumer_native_repo = repository_rule(
    implementation = _consumer_native_repo_impl,
)
