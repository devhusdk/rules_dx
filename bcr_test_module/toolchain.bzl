"""Reports the C and C++ toolchain a target platform resolves to."""

load("@bazel_tools//tools/cpp:toolchain_utils.bzl", "find_cpp_toolchain", "use_cpp_toolchain")

def _platform_impl(settings, attr):
    return {"//command_line_option:platforms": [attr.platform]}

_to_platform = transition(
    implementation = _platform_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, find_cpp_toolchain(ctx).toolchain_id + "\n")
    return DefaultInfo(files = depset([out]))

resolved_cc_toolchain = rule(
    implementation = _impl,
    attrs = {
        "platform": attr.string(mandatory = True),
    },
    cfg = _to_platform,
    fragments = ["cpp"],
    toolchains = use_cpp_toolchain(),
)
