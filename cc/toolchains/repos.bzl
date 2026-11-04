"""Module extension that provisions the pinned GCC toolchain."""

load("@gcc_toolchain//toolchain:defs.bzl", "gcc_declare_toolchain")
load(":pins.bzl", "GCC_GLIBC", "GCC_LINUX_X86_64", "GCC_LINUX_X86_64_TO_AARCH64", "GCC_VERSION")

# The sanitizer flags do nothing unless -fsanitize is on.
_LDFLAGS = ["-static-libasan", "-static-libubsan", "-lm"]

# The aarch64 cross toolchain has no dynamic_libstdcxx feature yet.
_CROSS_LDFLAGS = ["-static-libgcc", "-l:libstdc++.a"] + _LDFLAGS

def _gcc_impl(_mctx):
    gcc_declare_toolchain(
        name = "dx_gcc_linux_x86_64",
        target_arch = "x86_64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"x86_64": GCC_LINUX_X86_64}}),
        enable_fortran = False,
        target_libc = GCC_GLIBC,
        extra_ldflags = _LDFLAGS,
        extra_enabled_features = [str(Label("//cc/toolchains/gcc_features:dx_static_cxx_runtime"))],
        extra_known_features = [
            str(Label("//cc/toolchains/gcc_features:dynamic_libstdcxx")),
            str(Label("//cc/toolchains/gcc_features:static_link_cpp_runtimes")),
            str(Label("//cc/toolchains/gcc_features:static_linking_mode")),
            str(Label("//cc/toolchains/gcc_features:dynamic_linking_mode")),
        ],
    )
    gcc_declare_toolchain(
        name = "dx_gcc_linux_aarch64_cross",
        target_arch = "aarch64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"aarch64": GCC_LINUX_X86_64_TO_AARCH64}}),
        enable_fortran = False,
        target_libc = GCC_GLIBC,
        extra_ldflags = _CROSS_LDFLAGS,
    )
    return _mctx.extension_metadata(reproducible = True)

gcc = module_extension(
    implementation = _gcc_impl,
    doc = "Declares the pinned GCC Linux toolchain repositories (x86_64 and x86_64-to-aarch64 cross).",
)
