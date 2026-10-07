"""A consumer-side C++ greeting repository for the own-source wrapper fixture."""

_BUILD_BAZEL = '''load("@rules_dx//cc/rules:defs.bzl", "cc_library")

exports_files([
    "greeting.cc",
    "greeting.h",
])

cc_library(
    name = "greeting",
    srcs = ["greeting.cc"],
    hdrs = ["greeting.h"],
    includes = ["."],
    visibility = ["//visibility:public"],
)
'''

_GREETING_H = """#pragma once

namespace consumer_cc {

const char* greeting();

}  // namespace consumer_cc
"""

_GREETING_CC = '''#include "greeting.h"

namespace consumer_cc {

const char* greeting() {
  return "Hello from the consumer cc";
}

}  // namespace consumer_cc
'''

def _consumer_cc_repo_impl(ctx):
    """Writes the greeting repository a consumer of rules_dx would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("greeting.h", _GREETING_H)
    ctx.file("greeting.cc", _GREETING_CC)

consumer_cc_repo = repository_rule(
    implementation = _consumer_cc_repo_impl,
)
