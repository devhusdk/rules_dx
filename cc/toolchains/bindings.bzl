"""Authoritative C++ toolchain binding for hermetic linkage."""

CC_TOOLCHAIN_TYPE = "@bazel_tools//tools/cpp:toolchain_type"

def cc_toolchain_info(ctx):
    """Return the resolved C++ toolchain for the target platform."""
    return ctx.toolchains[CC_TOOLCHAIN_TYPE]

def cc_toolchain_toolchains():
    """Toolchain types a probe rule must declare for cc_toolchain_info."""
    return [CC_TOOLCHAIN_TYPE]
