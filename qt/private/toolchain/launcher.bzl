"""Writes the native launcher of a Qt tool and its descriptor."""

def _no_coverage_impl(_settings, _attr):
    return {"//command_line_option:collect_code_coverage": False}

_no_coverage = transition(
    implementation = _no_coverage_impl,
    inputs = [],
    outputs = ["//command_line_option:collect_code_coverage"],
)

NATIVE_LAUNCHER_ATTRS = {
    "_exec_launcher": attr.label(default = Label("//cli/launcher:exec_launcher"), executable = True, cfg = _no_coverage, doc = "The native launcher."),
    "_windows_os": attr.label(default = Label("@platforms//os:windows")),
}

def rlocation_path(ctx, file):
    """Returns the rlocation path of file, which starts with the repository name."""
    short_path = file.short_path
    if short_path.startswith("../"):
        return short_path[3:]
    return ctx.workspace_name + "/" + short_path

def write_native_launcher(ctx, descriptor_text):
    """Writes `<name>` on Unix or `<name>.exe` on Windows (the native launcher) and `<name>.launch` (its descriptor)."""
    windows = ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])
    launcher = ctx.actions.declare_file(ctx.label.name + (".exe" if windows else ""))
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
    ctx.actions.write(descriptor, descriptor_text)
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [launcher, descriptor]).merge(ctx.attr._exec_launcher[0][DefaultInfo].default_runfiles)
    return launcher, descriptor, runfiles
