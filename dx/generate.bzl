"""The gazelle runners of `//dx:generate` and `//dx:generate_check`."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")

visibility(["//dx/...", "//tools/..."])

GENERATE_EXCLUDES = [
    "-exclude=.dx",
    "-exclude=**/node_modules",
]

def _rlocation_path(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def gazelle_launch_descriptor(gazelle, mode, gazelle_args):
    """Returns the launcher descriptor that runs gazelle."""
    lines = ["program=" + gazelle, "verb=update", "verb=fix", "arg=-bzlmod"]
    defaults = ["-mode", mode] if mode else []
    lines.extend(["default_arg=" + arg for arg in defaults + gazelle_args])
    return "\n".join(lines) + "\n"

def _dx_gazelle_run_impl(ctx):
    descriptor = gazelle_launch_descriptor(
        gazelle = _rlocation_path(ctx, ctx.executable.gazelle),
        mode = ctx.attr.mode,
        gazelle_args = ctx.attr.gazelle_args,
    )
    launcher, _, launcher_runfiles = write_native_launcher(ctx, descriptor)
    runfiles = ctx.runfiles(files = [ctx.executable.gazelle])
    runfiles = runfiles.merge_all([launcher_runfiles, ctx.attr.gazelle[DefaultInfo].default_runfiles])
    return [DefaultInfo(executable = launcher, runfiles = runfiles)]

_dx_gazelle_run = rule(
    implementation = _dx_gazelle_run_impl,
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "gazelle": attr.label(mandatory = True, executable = True, cfg = "target", doc = "The gazelle binary."),
        "gazelle_args": attr.string_list(doc = "Arguments after the gazelle command."),
        "mode": attr.string(doc = "The gazelle -mode value."),
    },
    executable = True,
    doc = "Runs gazelle through the native launcher, so no shell tools and no Go SDK are needed.",
)

def dx_generate(name, mode = None, visibility = None):
    """A gazelle run target over the dx languages."""
    _dx_gazelle_run(
        name = name,
        gazelle = "@dx_gazelle//:gazelle",
        gazelle_args = GENERATE_EXCLUDES,
        mode = mode,
        visibility = visibility,
    )
