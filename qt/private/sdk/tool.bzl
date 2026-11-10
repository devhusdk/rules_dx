"""Launches one tool of a Qt cell through the shared native launcher."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")

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
    return "program=" + key + "\n" + "".join(["unset=" + name + "\n" for name in QT_TOOL_UNSET])

def _qt_tool_impl(ctx):
    program = ctx.file.program
    key = program.short_path[3:] if program.short_path.startswith("../") else ctx.workspace_name + "/" + program.short_path
    launcher, descriptor, launcher_runfiles = write_native_launcher(ctx, qt_tool_descriptor(key))
    runfiles = launcher_runfiles.merge(ctx.runfiles(files = [program]))
    for target in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(transitive_files = target[DefaultInfo].files)).merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = launcher, files = depset([launcher, descriptor]), runfiles = runfiles)]

qt_tool = rule(
    implementation = _qt_tool_impl,
    doc = "Runs a prebuilt Qt tool through the native launcher so it keeps its place beside its DLLs.",
    attrs = {
        "data": attr.label_list(allow_files = True, doc = "Files the tool needs at run time."),
        "program": attr.label(allow_single_file = True, mandatory = True, doc = "The tool executable inside the Qt cell."),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
)
