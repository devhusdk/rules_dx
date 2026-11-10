"""Runs a native test under kcov and writes its LCOV report for Bazel coverage."""

load("//core:rlocation.bzl", "rlocation_path")

def dx_kcov_attrs():
    """Returns the attributes kcov coverage needs."""
    return {
        "_dx_kcov": attr.label(
            default = Label("//tools/coverage/kcov:kcov_or_none"),
            cfg = "target",
        ),
        "_dx_kcov_launcher": attr.label(
            default = Label("//tools/coverage/kcov:kcov_launcher"),
            executable = True,
            cfg = "target",
        ),
        "_dx_kcov_linux": attr.label(
            default = Label("@platforms//os:linux"),
        ),
        "_dx_to_lcov": attr.label(
            default = Label("//tools/coverage/kcov:to_lcov_or_none"),
            cfg = "target",
        ),
    }


def dx_kcov_test_coverage(ctx, upstream, sources, dependency_attributes = ["upstream"]):
    """Returns coverage providers that run the upstream test under kcov over sources, or None off Linux."""
    if not ctx.target_platform_has_constraint(ctx.attr._dx_kcov_linux[platform_common.ConstraintValueInfo]):
        return None
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = dependency_attributes,
    )
    sources = [f for f in sources if f.is_source and not f.short_path.startswith("../")]
    manifest = ctx.actions.declare_file(ctx.label.name + ".kcov_sources")
    ctx.actions.write(manifest, "".join([f.short_path + "\n" for f in sources]))
    exe = upstream[DefaultInfo].files_to_run.executable
    kcov = ctx.attr._dx_kcov[DefaultInfo].files_to_run.executable
    to_lcov = ctx.attr._dx_to_lcov[DefaultInfo]
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    env.update({
        "DX_KCOV": rlocation_path(ctx, kcov),
        "DX_KCOV_SOURCES": rlocation_path(ctx, manifest),
        "DX_KCOV_UPSTREAM": rlocation_path(ctx, exe),
        "DX_TO_LCOV": rlocation_path(ctx, to_lcov.files_to_run.executable),
    })
    runfiles = ctx.runfiles(files = [manifest, exe, kcov] + sources).merge_all([
        upstream[DefaultInfo].default_runfiles,
        ctx.attr._dx_kcov[DefaultInfo].default_runfiles,
        to_lcov.default_runfiles,
        ctx.attr._dx_kcov_launcher[DefaultInfo].default_runfiles,
    ])
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = runfiles,
        executable = ctx.executable._dx_kcov_launcher,
    )
