"""The sorbet_tool rule: the pinned Sorbet binary as one executable."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def _sorbet_tool_impl(ctx):
    binary = ctx.file.binary
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "program=" + rlocation_path(ctx, binary) + "\n")
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = launcher_runfiles.merge(ctx.runfiles(files = [binary])),
    )]

sorbet_tool = rule(
    implementation = _sorbet_tool_impl,
    doc = "Runs the pinned Sorbet; the default of --@rules_dx//config:tool_sorbet.",
    executable = True,
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "binary": attr.label(allow_single_file = True, mandatory = True, doc = "The sorbet executable."),
    },
)
