# foreign_cmake fixture

Builds a pinned zlib through rules_foreign_cc `cmake` for Linux x86_64,
cross-compiled for Linux arm64 and, with cl.exe, for Windows x64.

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

`zlib_arm64_wrong_arch_link` runs the x86_64 toolchain's link command over the
arm64 `libz.a` with `--whole-archive` and keeps the linker output and exit code
in a log. `foreign_cmake_cross_test` checks that lld rejects the archive.

`zlib_log_gcc` rebuilds the same log with `compiler = "gcc"`.
`foreign_cmake_log_test` reads it through `zlib_log_gcc_copy` to check that
switching compilers reconfigures CMake instead of reusing a stale cache.

On windows_x86_64 with `--//cc:compiler=msvc`, `zlib_msvc_static` and
`zlib_msvc_shared` build the same zlib with cl.exe through the Ninja generator
(`zlibstatic.lib`, `zlib.lib` and `zlib.dll`; zlib only names the DLL `zlib1.dll`
under MinGW). The root MODULE.bazel uses the prebuilt ninja on Windows and noop
make, m4 and pkgconfig toolchains there. `foreign_cmake_msvc_log_test` checks the
MSVC identification and the /MD flag in the CMake log. Link warnings are not errors
for these probes (`/WX:NO`).
