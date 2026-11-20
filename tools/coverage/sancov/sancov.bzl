"""Runs a sancov-instrumented native test and writes its LCOV report for Bazel coverage."""

load("//core:no_coverage.bzl", "no_coverage_transition")
load("//core:rlocation.bzl", "rlocation_path")

def dx_sancov_attrs():
    """Returns the attributes sancov coverage needs."""
    return {
        "_dx_sancov_dsymutil": attr.label(
            default = Label("@llvm//tools:dsymutil"),
            allow_single_file = True,
            executable = True,
            cfg = "target",
        ),
        "_dx_sancov_launcher": attr.label(
            default = Label("//tools/coverage/sancov:sancov_launcher"),
            executable = True,
            cfg = "target",
        ),
        "_dx_sancov_macos": attr.label(
            default = Label("@platforms//os:macos"),
        ),
        "_dx_sancov_symbolizer": attr.label(
            default = Label("@llvm//tools:llvm-symbolizer"),
            allow_single_file = True,
            executable = True,
            cfg = "target",
        ),
        "_dx_sancov_to_lcov": attr.label(
            default = Label("//cli/lcov:to_lcov"),
            cfg = no_coverage_transition,
        ),
    }

def dx_sancov_test_coverage(ctx, upstream, sources, dependency_attributes = ["upstream"]):
    """Returns coverage providers that read the sancov PCs the upstream test writes, on macOS only."""
    if not ctx.target_platform_has_constraint(ctx.attr._dx_sancov_macos[platform_common.ConstraintValueInfo]):
        return None
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = dependency_attributes,
    )
    sources = [f for f in sources if f.is_source and not f.short_path.startswith("../")]
    manifest = ctx.actions.declare_file(ctx.label.name + ".sancov_sources")
    ctx.actions.write(manifest, "".join([f.short_path + "\n" for f in sources]))
    exe = upstream[DefaultInfo].files_to_run.executable
    to_lcov = ctx.attr._dx_sancov_to_lcov[0][DefaultInfo]
    symbolizer = ctx.file._dx_sancov_symbolizer
    dsymutil = ctx.file._dx_sancov_dsymutil
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    env.update({
        "DX_DSYMUTIL": rlocation_path(ctx, dsymutil),
        "DX_LLVM_SYMBOLIZER": rlocation_path(ctx, symbolizer),
        "DX_SANCOV_SOURCES": rlocation_path(ctx, manifest),
        "DX_SANCOV_UPSTREAM": rlocation_path(ctx, exe),
        "DX_TO_LCOV": rlocation_path(ctx, to_lcov.files_to_run.executable),
    })
    runfiles = ctx.runfiles(files = [manifest, exe, symbolizer, dsymutil] + sources).merge_all([
        upstream[DefaultInfo].default_runfiles,
        to_lcov.default_runfiles,
        ctx.attr._dx_sancov_launcher[DefaultInfo].default_runfiles,
    ])
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = runfiles,
        executable = ctx.executable._dx_sancov_launcher,
    )
