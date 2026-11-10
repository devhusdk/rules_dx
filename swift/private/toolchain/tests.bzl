"""Analysis tests for the Swift Linux toolchains."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_swift//swift:providers.bzl", "SwiftToolchainInfo")

def _link_flags(cc_info):
    return [
        flag
        for linker_input in cc_info.linking_context.linker_inputs.to_list()
        for flag in linker_input.user_link_flags
    ]

def _cross_runtime_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[SwiftToolchainInfo]
    target_runtime = ctx.attr.target_root + "/usr/lib/swift"
    exec_runtime = ctx.attr.exec_root + "/usr/lib/swift"
    for name in ("dynamic_runtime_cc_info", "static_runtime_cc_info"):
        flags = _link_flags(getattr(info, name))
        asserts.true(env, [flag for flag in flags if target_runtime in flag], name + " has no target runtime flag: " + str(flags))
        asserts.equals(env, [], [flag for flag in flags if exec_runtime in flag], name + " links the runtime of the exec cpu")
    asserts.equals(env, ctx.attr.exec_root + "/usr", info.root_dir)
    return analysistest.end(env)

_cross_runtime_test = analysistest.make(
    _cross_runtime_impl,
    attrs = {
        "exec_root": attr.string(mandatory = True, doc = "The root of the Swift archive the tools run from."),
        "target_root": attr.string(mandatory = True, doc = "The root of the Swift archive of the target cpu."),
    },
)

def swift_cross_runtime_test(name, exec_cpu, target_cpu, **kwargs):
    """Asserts a cross toolchain links the Swift runtime of the target cpu and runs the tools of the exec cpu.

    Args:
        name: The test name.
        exec_cpu: The cpu the Swift tools run on.
        target_cpu: The cpu the Swift tools build for.
        **kwargs: Passed to the test.
    """
    _cross_runtime_test(
        name = name,
        size = "small",
        exec_root = Label("@dx_swift_linux_%s//:files" % exec_cpu).workspace_root,
        target_root = Label("@dx_swift_linux_%s//:files" % target_cpu).workspace_root,
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + target_cpu],
        target_under_test = "//swift/private/toolchain:swift_toolchain_%s_%s" % (exec_cpu, target_cpu),
        **kwargs
    )
