"""Launches one tool of a Qt cell through the shared native launcher."""

load(":launcher.bzl", "NATIVE_LAUNCHER_ATTRS", "rlocation_path", "write_native_launcher")

QT_TOOL_UNSET = [
    "DYLD_FALLBACK_FRAMEWORK_PATH",
    "DYLD_FALLBACK_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "QML2_IMPORT_PATH",
    "QML_IMPORT_PATH",
    "QT_CONF_PATH",
    "QT_PLUGIN_PATH",
    "QT_QPA_PLATFORM_PLUGIN_PATH",
]

def qt_tool_descriptor(key, library = ""):
    """Returns the launch descriptor text for a tool key and the key of a library whose directory joins LD_LIBRARY_PATH."""
    text = "program=" + key + "\n"
    if library:
        text += "env_dir=LD_LIBRARY_PATH\t" + library + "\n"
    return text + "".join(["unset=" + name + "\n" for name in QT_TOOL_UNSET])

def _qt_tool_impl(ctx):
    program = ctx.file.program
    library = ctx.file.library
    files = [program] + ([library] if library else [])
    descriptor_text = qt_tool_descriptor(rlocation_path(ctx, program), rlocation_path(ctx, library) if library else "")
    launcher, descriptor, launcher_runfiles = write_native_launcher(ctx, descriptor_text)
    runfiles = launcher_runfiles.merge(ctx.runfiles(files = files))
    for target in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(transitive_files = target[DefaultInfo].files)).merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = launcher, files = depset([launcher, descriptor]), runfiles = runfiles)]

qt_tool = rule(
    implementation = _qt_tool_impl,
    doc = "Runs a prebuilt Qt tool through the native launcher so it keeps its place beside its DLLs.",
    attrs = {
        "data": attr.label_list(allow_files = True, doc = "Files the tool needs at run time."),
        "library": attr.label(allow_single_file = True, doc = "A library in the cell's lib directory, which the launcher puts on LD_LIBRARY_PATH."),
        "program": attr.label(allow_single_file = True, mandatory = True, doc = "The tool executable inside the Qt cell."),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
)
