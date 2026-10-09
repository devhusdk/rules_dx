"""A consumer-owned native application repository packaged outside the main graph."""

_BUILD_BAZEL = '''load("@rules_cc//cc:defs.bzl", "cc_binary", "cc_library")

exports_files(["NOTICE.txt", "banner.txt"])

cc_library(
    name = "external_greet",
    srcs = ["external_greet.cc"],
    hdrs = ["external_greet.h"],
    copts = ["-std=c++17"],
    includes = ["."],
    visibility = ["//visibility:public"],
)

cc_binary(
    name = "external_app",
    srcs = ["external_main.cc"],
    copts = ["-std=c++17"],
    visibility = ["//visibility:public"],
    deps = [":external_greet"],
)
'''

_GREET_H = '''#pragma once

#ifdef __cplusplus
extern "C" {
#endif

typedef struct external_greet_state external_greet_state;

external_greet_state* external_greet_create(const char* name);
const char* external_greet_message(external_greet_state* state);
void external_greet_destroy(external_greet_state* state);

#ifdef __cplusplus
}
#endif
'''

_GREET_CC = '''#include "external_greet.h"

#include <string>

struct external_greet_state {
  std::string message;
};

external_greet_state* external_greet_create(const char* name) {
  const std::string who = (name != nullptr) ? name : "world";
  external_greet_state* state = new external_greet_state();
  state->message = "Hello from the external native library, " + who;
  return state;
}

const char* external_greet_message(external_greet_state* state) {
  if (state == nullptr) {
    return "";
  }
  return state->message.c_str();
}

void external_greet_destroy(external_greet_state* state) {
  delete state;
}
'''

_MAIN_CC = '''#include <fstream>
#include <iostream>
#include <string>

#include "external_greet.h"

namespace {

std::string DirName(const std::string& path) {
  size_t pos = path.find_last_of("/\\\\");
  if (pos == std::string::npos) {
    return ".";
  }
  if (pos == 0) {
    return path.substr(0, 1);
  }
  return path.substr(0, pos);
}

}  // namespace

int main(int argc, char** argv) {
  std::string exe = (argc > 0 && argv[0] != nullptr) ? argv[0] : ".";
  std::string banner = DirName(exe) + "/resources/banner.txt";
  std::ifstream in(banner);
  if (!in) {
    std::cerr << "external_app: cannot read " << banner << "\\n";
    return 1;
  }
  external_greet_state* state = external_greet_create("external");
  if (state == nullptr) {
    std::cerr << "external_app: native create failed\\n";
    return 1;
  }
  std::string line;
  while (std::getline(in, line)) {
    std::cout << "external_app resource: " << line << "\\n";
  }
  std::cout << "external_app native: " << external_greet_message(state) << "\\n";
  external_greet_destroy(state);
  return 0;
}
'''

_BANNER_TXT = """dx-external-archive-marker
banner-external-v1
"""

_NOTICE_TXT = """external-release NOTICE
Third-party licenses for the external consumer application.
"""

def _consumer_archive_repo_impl(ctx):
    """Writes the application, library, resource and notice a consumer would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("external_greet.h", _GREET_H)
    ctx.file("external_greet.cc", _GREET_CC)
    ctx.file("external_main.cc", _MAIN_CC)
    ctx.file("banner.txt", _BANNER_TXT)
    ctx.file("NOTICE.txt", _NOTICE_TXT)

consumer_archive_repo = repository_rule(
    implementation = _consumer_archive_repo_impl,
)
