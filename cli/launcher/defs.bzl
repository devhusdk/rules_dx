"""Shared pieces for rules that run the native launcher on Windows."""

NATIVE_LAUNCHER_ATTRS = {
    "_exec_launcher": attr.label(default = Label("//cli/launcher:exec_launcher"), executable = True, cfg = "target", doc = "The native launcher used on Windows."),
    "_windows_os": attr.label(default = Label("@platforms//os:windows")),
}

def target_is_windows(ctx):
    """Returns whether the rule builds for a Windows target platform."""
    return ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])

def native_launch_descriptor_path(exe_path):
    """Returns the descriptor path of the native launcher at `exe_path`: `x.exe` becomes `x.launch`."""
    if not exe_path.endswith(".exe"):
        fail("not a native launcher: " + exe_path)
    return exe_path[:-len(".exe")] + ".launch"

def write_native_launcher(ctx, descriptor_text):
    """Writes `<name>.exe` (the native launcher) and `<name>.launch` (its descriptor).

    Returns the executable, the descriptor and the runfiles the launcher needs.
    """
    launcher = ctx.actions.declare_file(ctx.label.name + ".exe")
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
    ctx.actions.write(descriptor, descriptor_text)
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [launcher, descriptor]).merge(ctx.attr._exec_launcher[DefaultInfo].default_runfiles)
    return launcher, descriptor, runfiles
