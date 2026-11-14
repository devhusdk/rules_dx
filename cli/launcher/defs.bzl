"""Shared pieces for rules that run the native launcher."""

load("//core:no_coverage.bzl", "no_coverage_transition")

NATIVE_LAUNCHER_ATTRS = {
    "_exec_launcher": attr.label(default = Label("//cli/launcher:exec_launcher"), executable = True, cfg = no_coverage_transition, doc = "The native launcher."),
    "_windows_os": attr.label(default = Label("@platforms//os:windows")),
}

def target_is_windows(ctx):
    """Returns whether the rule builds for a Windows target platform."""
    return ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])

def native_launch_descriptor_path(exe_path):
    """Returns the descriptor path of the native launcher at `exe_path`: `x.exe` and `x` become `x.launch`."""
    if exe_path.endswith(".exe"):
        return exe_path[:-len(".exe")] + ".launch"
    return exe_path + ".launch"

def write_native_launcher(ctx, descriptor_text, launcher_name = None):
    """Writes `<name>` on Unix or `<name>.exe` on Windows (the native launcher) and `<name>.launch` (its descriptor)."""
    launcher = ctx.actions.declare_file((launcher_name or ctx.label.name) + (".exe" if target_is_windows(ctx) else ""))
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
    ctx.actions.write(descriptor, descriptor_text)
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [launcher, descriptor]).merge(ctx.attr._exec_launcher[0][DefaultInfo].default_runfiles)
    return launcher, descriptor, runfiles
