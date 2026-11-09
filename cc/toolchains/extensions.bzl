"""Module extension for cc/toolchains."""

load("@gcc_toolchain//toolchain:defs.bzl", "gcc_declare_toolchain")
load(":pins.bzl", "GCC_GLIBC", "GCC_LINUX_AARCH64", "GCC_LINUX_X86_64", "GCC_LINUX_X86_64_TO_AARCH64", "GCC_VERSION")


_LDFLAGS = ["-static-libasan", "-static-libubsan", "-lm"]


_EXTRA_ENABLED = [str(Label("//cc/toolchains/gcc_features:dx_static_cxx_runtime"))]


_EXTRA_KNOWN = [
    str(Label("//cc/toolchains/gcc_features:dynamic_libstdcxx")),
    str(Label("//cc/toolchains/gcc_features:static_link_cpp_runtimes")),
    str(Label("//cc/toolchains/gcc_features:static_linking_mode")),
    str(Label("//cc/toolchains/gcc_features:dynamic_linking_mode")),
]


def _gcc_impl(_mctx):
    gcc_declare_toolchain(
        name = "dx_gcc_linux_x86_64",
        target_arch = "x86_64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"x86_64": GCC_LINUX_X86_64}}),
        enable_fortran = False,
        target_libc = GCC_GLIBC,
        extra_ldflags = _LDFLAGS,
        extra_enabled_features = _EXTRA_ENABLED,
        extra_known_features = _EXTRA_KNOWN,
    )
    gcc_declare_toolchain(
        name = "dx_gcc_linux_aarch64_cross",
        target_arch = "aarch64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"aarch64": GCC_LINUX_X86_64_TO_AARCH64}}),
        enable_fortran = False,
        target_libc = GCC_GLIBC,
        extra_ldflags = _LDFLAGS,
        extra_enabled_features = _EXTRA_ENABLED,
        extra_known_features = _EXTRA_KNOWN,
    )
    gcc_declare_toolchain(
        name = "dx_gcc_linux_aarch64",
        target_arch = "aarch64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"aarch64": GCC_LINUX_AARCH64}}),
        enable_fortran = False,
        target_libc = GCC_GLIBC,
        extra_ldflags = _LDFLAGS,
        extra_enabled_features = _EXTRA_ENABLED,
        extra_known_features = _EXTRA_KNOWN,
    )
    return _mctx.extension_metadata(reproducible = True)


gcc = module_extension(
    implementation = _gcc_impl,
    doc = "Declares the pinned GCC Linux toolchain repositories (x86_64, x86_64-to-aarch64 cross and native aarch64).",
)
