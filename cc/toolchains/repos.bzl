"""Module extension that provisions the pinned GCC toolchain."""

load("@gcc_toolchain//toolchain:defs.bzl", "gcc_declare_toolchain")
load(":pins.bzl", "GCC_LINUX_X86_64", "GCC_LINUX_X86_64_TO_AARCH64", "GCC_VERSION")

# The sanitizer flags do nothing unless -fsanitize is on.
_LDFLAGS = ["-static-libgcc", "-static-libasan", "-static-libubsan", "-l:libstdc++.a", "-lm"]

def _gcc_impl(_mctx):
    gcc_declare_toolchain(
        name = "dx_gcc_linux_x86_64",
        target_arch = "x86_64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"x86_64": GCC_LINUX_X86_64}}),
        enable_fortran = False,
        extra_ldflags = _LDFLAGS,
    )
    gcc_declare_toolchain(
        name = "dx_gcc_linux_aarch64_cross",
        target_arch = "aarch64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"aarch64": GCC_LINUX_X86_64_TO_AARCH64}}),
        enable_fortran = False,
        extra_ldflags = _LDFLAGS,
    )
    return _mctx.extension_metadata(reproducible = True)

gcc = module_extension(
    implementation = _gcc_impl,
    doc = "Declares the pinned GCC Linux toolchain repositories (x86_64 and x86_64-to-aarch64 cross).",
)
