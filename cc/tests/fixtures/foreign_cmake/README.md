# foreign_cmake fixture

Builds a pinned zlib through rules_foreign_cc `cmake` for Linux x86_64 and,
cross-compiled, for Linux arm64.

The cmake, ninja and C toolchains come from Bazel toolchains. The one ambient
tool is `bash`: rules_foreign_cc runs its generated build script with the host
shell and has no shell toolchain to declare it through.

Not covered yet (part of #6222): analysis failure tests for a missing cmake
toolchain, an unsupported generator and a wrong-architecture `cc_import`.
