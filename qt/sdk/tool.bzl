"""Launches one tool of a Qt cell through the shared native launcher."""

# Mirrors the non-DYLD entries of host_routes in launcher.c.
QT_TOOL_UNSET = [
    "QML2_IMPORT_PATH",
    "QML_IMPORT_PATH",
    "QT_CONF_PATH",
    "QT_PLUGIN_PATH",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
]

def qt_tool_descriptor(key):
    """Returns the launch descriptor text for a tool key."""
    return "version=1\nprogram=" + key + "\n" + "".join(["unset=" + name + "\n" for name in QT_TOOL_UNSET])

def _qt_tool_impl(ctx):
    program = ctx.file.program
    key = program.short_path[3:] if program.short_path.startswith("../") else ctx.workspace_name + "/" + program.short_path
    launcher = ctx.actions.declare_file(ctx.label.name + ".exe")
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
    ctx.actions.write(descriptor, qt_tool_descriptor(key))
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [launcher, descriptor, program]).merge(ctx.attr._exec_launcher[DefaultInfo].default_runfiles)
    for target in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(transitive_files = target[DefaultInfo].files)).merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = launcher, files = depset([launcher, descriptor]), runfiles = runfiles)]

qt_tool = rule(
    implementation = _qt_tool_impl,
    doc = "Runs a prebuilt Qt tool through the native launcher so it keeps its place beside its DLLs.",
    attrs = {
        "data": attr.label_list(allow_files = True, doc = "Files the tool needs at run time."),
        "program": attr.label(allow_single_file = True, mandatory = True, doc = "The tool executable inside the Qt cell."),
        "_exec_launcher": attr.label(default = Label("//libs/launcher:exec_launcher"), executable = True, cfg = "target"),
    },
    executable = True,
)
