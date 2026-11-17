"""The cc extension: the pinned GCC, MSVC and Windows SDK repositories and the dx_cc_toolchains hub."""

load("@dx_windows_support//windows/private/extensions:msvc_runtime.bzl", "MSVC_RUNTIME_CONFIGURE_ATTRS", "msvc_runtime_repository")
load("@dx_windows_support//windows/private/extensions:windows_sdk.bzl", "WINDOWS_SDK_CONFIGURE_ATTRS", "windows_sdk_repository")
load("//cc/private/toolchain/gcc_repo:defs.bzl", "gcc_declare_toolchain")
load("//cc/private/toolchain/msvc:extensions.bzl", "msvc_hub")
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

_HOSTS = ["arm64", "x64"]

def _tag(mctx, name):
    found = None
    for mod in mctx.modules:
        tags = getattr(mod.tags, name)
        if len(tags) > 1:
            fail("Only one cc.{}(...) tag is supported per module.".format(name))
        if tags and (mod.is_root or found == None):
            found = tags[0]
        if tags and mod.is_root:
            break
    if found == None:
        fail("Missing cc.{}(...) in MODULE.bazel".format(name))
    return found

def _host_tools(mctx):
    hosts = []
    for mod in mctx.modules:
        for tag in mod.tags.host_tools:
            for host in tag.hosts:
                if host not in hosts:
                    hosts.append(host)
    return sorted(hosts) if hosts else _HOSTS

def _cc_impl(mctx):
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
    hosts = _host_tools(mctx)
    msvc_runtime = msvc_runtime_repository(mctx, "dx_msvc_runtime", _tag(mctx, "msvc_runtime"), hosts)
    windows_sdk = windows_sdk_repository(mctx, "dx_windows_sdk", _tag(mctx, "windows_sdk"), hosts)
    msvc_hub(mctx)
    _hub(name = "dx_cc_toolchains", build = cc_hub())
    return mctx.extension_metadata(
        facts = msvc_runtime.facts,
        reproducible = msvc_runtime.reproducible and windows_sdk.reproducible,
    )


cc = module_extension(
    implementation = _cc_impl,
    tag_classes = {
        "host_tools": tag_class(
            attrs = {
                "hosts": attr.string_list(
                    mandatory = True,
                    doc = "Host architectures (`x64`, `arm64`) whose MSVC and Windows SDK tools are kept. Values from all modules are combined.",
                ),
            },
            doc = "Keeps the MSVC and Windows SDK tools for the given hosts only.",
        ),
        "msvc_runtime": tag_class(
            attrs = MSVC_RUNTIME_CONFIGURE_ATTRS,
            doc = "Configures the dx_msvc_runtime repository.",
        ),
        "windows_sdk": tag_class(
            attrs = WINDOWS_SDK_CONFIGURE_ATTRS,
            doc = "Configures the dx_windows_sdk repository.",
        ),
    },
    doc = "Declares the pinned GCC Linux, MSVC and Windows SDK repositories and the dx_cc_toolchains and dx_msvc hubs.",
)
