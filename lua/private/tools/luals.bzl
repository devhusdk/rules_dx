"""The luals_tool rule: the pinned LuaLS release tree as one executable."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def _luals_tool_impl(ctx):
    binary = ctx.file.binary
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "real_program=" + rlocation_path(ctx, binary) + "\n")
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = launcher_runfiles.merge(ctx.runfiles(transitive_files = depset(ctx.files.tree))),
    )]

luals_tool = rule(
    implementation = _luals_tool_impl,
    doc = "Runs the pinned LuaLS on its own release tree; the default of --@rules_dx//config:tool_luals.",
    executable = True,
    attrs = dict({
        "binary": attr.label(allow_single_file = True, mandatory = True, doc = "The LuaLS executable inside the release tree."),
        "tree": attr.label(mandatory = True, allow_files = True, doc = "Every file of the release tree: main.lua, script/, meta/ and locale/ must sit next to the binary."),
    }, **NATIVE_LAUNCHER_ATTRS),
)
