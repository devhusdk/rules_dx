"""Rule that exposes a JDK of a pinned version to one target."""

load("@rules_java//java/common:java_common.bzl", "java_common")


def _version_transition_impl(_settings, attr):
    return {"//command_line_option:java_runtime_version": "remotejdk_" + attr.version}

_version_transition = transition(
    implementation = _version_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:java_runtime_version"],
)

def _impl(ctx):
    runtime = ctx.attr._runtime[0][java_common.JavaRuntimeInfo]
    files = runtime.files
    return [
        DefaultInfo(
            files = files,
            runfiles = ctx.runfiles(transitive_files = files),
        ),
        runtime,
        platform_common.TemplateVariableInfo({
            "JAVA_ROOTPATH": runtime.java_executable_runfiles_path,
        }),
    ]

pinned_java_runtime = rule(
    implementation = _impl,
    doc = "Provides the JDK of `version` regardless of the repo-wide runtime.",
    attrs = {
        "version": attr.string(mandatory = True, doc = "Major JDK version, for example 25."),
        "_runtime": attr.label(
            default = "@bazel_tools//tools/jdk:current_java_runtime",
            cfg = _version_transition,
            providers = [java_common.JavaRuntimeInfo],
        ),
    },
)
