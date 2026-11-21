"""A py_cc_toolchain that sets the target platform on a rules_py interpreter's py_cc_toolchain."""

# Upstream: aspect-build/rules_py#1665
# Once a rules_py release has it, delete this file, its load and dx_py_cc_toolchain targets in
# interpreters.bzl, and point the _py_cc toolchains there back at @<repo>//:py_cc_toolchain.

load("@rules_python//python/cc:py_cc_toolchain_info.bzl", "PyCcToolchainInfo")

def _dx_py_cc_toolchain_impl(ctx):
    base = ctx.attr.base[platform_common.ToolchainInfo].py_cc_toolchain
    major, minor = base.python_version.split(".")[:2]
    prefix = "cp" if ctx.attr.sys_platform == "win32" else "cpython-"
    return [platform_common.ToolchainInfo(py_cc_toolchain = PyCcToolchainInfo(
        abi_flags = base.abi_flags,
        headers = base.headers,
        headers_abi3 = base.headers_abi3,
        libs = base.libs,
        platform_machine = ctx.attr.platform_machine,
        platform_tag = ctx.attr.platform_tag,
        python_version = base.python_version,
        soabi = "{}{}{}{}-{}".format(prefix, major, minor, base.abi_flags, ctx.attr.platform_tag),
        sys_platform = ctx.attr.sys_platform,
    ))]

dx_py_cc_toolchain = rule(
    implementation = _dx_py_cc_toolchain_impl,
    doc = "Returns the base py_cc_toolchain's headers and libs with the given target platform and its SOABI.",
    attrs = {
        "base": attr.label(mandatory = True, providers = [platform_common.ToolchainInfo], doc = "A py_cc_toolchain target."),
        "platform_machine": attr.string(mandatory = True, doc = "PEP 508 platform_machine of the target, e.g. aarch64."),
        "platform_tag": attr.string(mandatory = True, doc = "PEP 3149 platform tag of the target, e.g. win_amd64."),
        "sys_platform": attr.string(mandatory = True, doc = "PEP 508 sys_platform of the target, e.g. linux."),
    },
)
