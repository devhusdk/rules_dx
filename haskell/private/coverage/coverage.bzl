"""LCOV coverage for Haskell test forwarders, converted from hpc."""

load("//haskell/private/toolchain:transition.bzl", "ghc_mingw_transition")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def dx_haskell_coverage_attrs():
    """Returns the attributes Haskell coverage needs."""
    return NATIVE_LAUNCHER_ATTRS | {
        "_dx_hpc_coverage": attr.label(
            default = Label("//cli/hpc_coverage"),
            executable = True,
            cfg = "target",
        ),
        "_dx_hpc_lcov": attr.label(
            default = Label("//haskell/private/coverage:hpc_lcov"),
            executable = True,
            cfg = ghc_mingw_transition,
        ),
    }


def dx_haskell_test_coverage(ctx, upstream):
    """Returns coverage providers that convert the upstream test's hpc data to LCOV."""
    upstream_exe = upstream[DefaultInfo].files_to_run.executable
    lcov = ctx.attr._dx_hpc_lcov[0][DefaultInfo]
    runner = ctx.attr._dx_hpc_coverage[DefaultInfo]
    launcher, _, launcher_runfiles = write_native_launcher(
        ctx,
        "program={}\npath={}\npath={}\n".format(
            rlocation_path(ctx, runner.files_to_run.executable),
            rlocation_path(ctx, upstream_exe),
            rlocation_path(ctx, lcov.files_to_run.executable),
        ),
        launcher_name = ctx.label.name + "_coverage/" + ctx.label.name + "_upstream",
    )
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream"],
    )
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = [upstream_exe, runner.files_to_run.executable]).merge_all([lcov.default_runfiles, runner.default_runfiles, launcher_runfiles]),
        executable = launcher,
    )
