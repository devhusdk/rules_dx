"""Shared pieces for rules that run the native launcher."""

load("//core:no_coverage.bzl", "no_coverage_transition")
load("//core:rlocation.bzl", "rlocation_path")

NATIVE_LAUNCHER_ATTRS = {
    "_exec_launcher": attr.label(default = Label("@dx_prebuilt//:launcher"), executable = True, cfg = no_coverage_transition, doc = "The native launcher."),
    "_windows_os": attr.label(default = Label("@platforms//os:windows")),
}

def target_is_windows(ctx):
    """Returns whether the rule builds for a Windows target platform."""
    return ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])

def native_launch_descriptor_path(exe_path):
    """Returns the launcher descriptor path for `exe_path`."""
    if exe_path.endswith(".exe"):
        return exe_path[:-len(".exe")] + ".launch"
    return exe_path + ".launch"

def write_native_launcher(ctx, descriptor_text, launcher_name = None):
    """Writes the native launcher and its descriptor."""
    launcher = ctx.actions.declare_file((launcher_name or ctx.label.name) + (".exe" if target_is_windows(ctx) else ""))
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
    ctx.actions.write(descriptor, descriptor_text)
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [launcher, descriptor]).merge(ctx.attr._exec_launcher[0][DefaultInfo].default_runfiles)
    return launcher, descriptor, runfiles

def subcommand_descriptor(program_key, subcommand):
    """Returns the launch descriptor text that runs `program_key subcommand`."""
    return "program=" + program_key + "\narg=" + subcommand + "\n"

def _dx_subcommand_impl(ctx):
    dx = ctx.executable._dx
    launcher, _, runfiles = write_native_launcher(ctx, subcommand_descriptor(rlocation_path(ctx, dx), ctx.attr.subcommand))
    runfiles = runfiles.merge(ctx.runfiles(files = [dx])).merge(ctx.attr._dx[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

dx_subcommand = rule(
    implementation = _dx_subcommand_impl,
    doc = "Runs `dx <subcommand> [ARG]...` from the prebuilt dx through the native launcher.",
    executable = True,
    attrs = dict({
        "subcommand": attr.string(mandatory = True, doc = "The dx multicall subcommand."),
        "_dx": attr.label(default = Label("@dx_prebuilt//:dx"), executable = True, cfg = "target", doc = "The prebuilt dx."),
    }, **NATIVE_LAUNCHER_ATTRS),
)
