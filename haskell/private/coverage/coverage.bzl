"""LCOV coverage for Haskell test forwarders, converted from hpc."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:no_coverage.bzl", "no_coverage_transition")
load("//core:rlocation.bzl", "rlocation_path")
load("//haskell/private/toolchain:transition.bzl", "ghc_mingw_transition")

def dx_haskell_coverage_attrs():
    """Returns the attributes Haskell coverage needs."""
    return NATIVE_LAUNCHER_ATTRS | {
        "_dx_hpc_coverage": attr.label(
            default = Label("//cli/runtime:dx_runtime"),
            executable = True,
            cfg = no_coverage_transition,
        ),
        "_dx_hpc_lcov": attr.label(
            default = Label("//haskell/private/coverage:hpc_lcov"),
            executable = True,
            cfg = ghc_mingw_transition,
        ),
    }

def dx_haskell_test_coverage(ctx, upstream):
    """Returns coverage providers that convert the upstream test's hpc data to LCOV."""
    binaries = [f for f in upstream[DefaultInfo].files.to_list() if f != upstream[DefaultInfo].files_to_run.executable]
    upstream_exe = binaries[0] if binaries else upstream[DefaultInfo].files_to_run.executable
    mix = [f for f in upstream[DefaultInfo].default_runfiles.files.to_list() if f.extension == "mix"]
    lcov = ctx.attr._dx_hpc_lcov[0][DefaultInfo]
    runner = ctx.attr._dx_hpc_coverage[0][DefaultInfo]
    descriptor = "program={}\nenv=DX_RUNTIME_TOOL\thpc_coverage\npath={}\npath={}\n".format(
        rlocation_path(ctx, runner.files_to_run.executable),
        rlocation_path(ctx, upstream_exe),
        rlocation_path(ctx, lcov.files_to_run.executable),
    )
    descriptor += "".join(["path={}\n".format(rlocation_path(ctx, f)) for f in mix])
    descriptor += "arg=--\n"
    launcher, _, launcher_runfiles = write_native_launcher(
        ctx,
        descriptor,
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
        runfiles = ctx.runfiles(files = [upstream_exe, runner.files_to_run.executable] + mix).merge_all([upstream[DefaultInfo].default_runfiles, lcov.default_runfiles, runner.default_runfiles, launcher_runfiles]),
        executable = launcher,
    )
