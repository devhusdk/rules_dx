"""Fails analysis unless the target platform resolves to the expected C and C++ toolchain."""

load("@bazel_tools//tools/cpp:toolchain_utils.bzl", "find_cpp_toolchain", "use_cpp_toolchain")

def _impl(ctx):
    toolchain_id = find_cpp_toolchain(ctx).toolchain_id
    if toolchain_id != ctx.attr.expected:
        fail("{} resolved the C++ toolchain {}, want {}".format(ctx.label, toolchain_id, ctx.attr.expected))
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, toolchain_id + "\n")
    return DefaultInfo(files = depset([out]))

resolved_cc_toolchain = rule(
    implementation = _impl,
    attrs = {
        "expected": attr.string(mandatory = True),
    },
    fragments = ["cpp"],
    toolchains = use_cpp_toolchain(),
)
