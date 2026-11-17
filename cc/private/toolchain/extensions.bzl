"""The cc extension: the pinned GCC repositories and the dx_cc_toolchains hub."""

load("//cc/private/toolchain/gcc_repo:defs.bzl", "gcc_declare_toolchain")
load(":hub.bzl", "cc_hub")
load(":pins.bzl", "GCC_GLIBC", "GCC_LINUX_AARCH64", "GCC_LINUX_X86_64", "GCC_LINUX_X86_64_TO_AARCH64", "GCC_VERSION")


_LDFLAGS = ["-static-libasan", "-static-libtsan", "-static-libubsan", "-lm"]


_EXTRA_ENABLED = [str(Label("//cc/private/toolchain/gcc_features:dx_static_cxx_runtime"))]


_EXTRA_KNOWN = [
    str(Label("//cc/private/toolchain/gcc_features:dynamic_libstdcxx")),
    str(Label("//cc/private/toolchain/gcc_features:static_link_cpp_runtimes")),
    str(Label("//cc/private/toolchain/gcc_features:static_linking_mode")),
    str(Label("//cc/private/toolchain/gcc_features:dynamic_linking_mode")),
]


def _hub_impl(rctx):
    rctx.file("BUILD.bazel", rctx.attr.build)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Writes the BUILD file of the C and C++ toolchain hub.",
    attrs = {"build": attr.string(mandatory = True, doc = "BUILD file content of the hub.")},
)

def _cc_impl(_mctx):
    gcc_declare_toolchain(
        name = "dx_gcc_linux_x86_64",
        target_arch = "x86_64",
        gcc_version = GCC_VERSION,
        gcc_versions = json.encode({GCC_VERSION: {"x86_64": GCC_LINUX_X86_64}}),
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
        target_libc = GCC_GLIBC,
        extra_ldflags = _LDFLAGS,
        extra_enabled_features = _EXTRA_ENABLED,
        extra_known_features = _EXTRA_KNOWN,
    )
    _hub(name = "dx_cc_toolchains", build = cc_hub())
    return _mctx.extension_metadata(reproducible = True)


cc = module_extension(
    implementation = _cc_impl,
    doc = "Declares the pinned GCC Linux toolchain repositories and the dx_cc_toolchains hub.",
)
