# foreign_cmake fixture

Builds a pinned zlib through rules_foreign_cc `cmake` for Linux x86_64 and,
cross-compiled, for Linux arm64.

The cmake, ninja and C toolchains come from Bazel toolchains. The one ambient
tool is `bash`: rules_foreign_cc runs its generated build script with the host
shell and has no shell toolchain to declare it through.

The root MODULE.bazel uses `tools.explicit()` with pinned cmake, ninja, make,
m4 and pkgconfig, so rules_foreign_cc has no host cmake fallback. The arm64
cell runs cmake with `--trace-expand` so its log shows `CMAKE_CROSSCOMPILING`.

`unknown_generator_test` checks that `-GBogus` fails analysis with a clear
message and no fallback generator.

`missing_cmake_toolchain_test` resolves an optional cmake toolchain probe on a
riscv64 exec platform with no pinned cmake and expects rules_foreign_cc's "No
toolchain found" failure, so `tools.explicit()` has no system fallback.
`cmake_toolchain_present_test` is the positive control on the host. A real
`cmake` target on riscv64 would fail earlier on the LLVM glibc stubs select.

Not covered yet (part of #6222): a wrong-architecture `cc_import`, tracked in
#7246.
