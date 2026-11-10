"""The dx_ldmd2 rule: ldmd2 started by the native launcher with the runtime closure on its library path."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def ldmd2_descriptor(compiler_key, closure_key):
    """Returns the launch descriptor text; closure_key is empty when there is no runtime closure."""
    text = "program=" + compiler_key + "\n"
    if closure_key:
        text += "env_dir=LD_LIBRARY_PATH\t" + closure_key + "\n"
    return text

def _dx_ldmd2_impl(ctx):
    closure = ctx.file.closure_marker
    launcher, _, launcher_runfiles = write_native_launcher(
        ctx,
        ldmd2_descriptor(rlocation_path(ctx, ctx.file.compiler), rlocation_path(ctx, closure) if closure else ""),
    )
    runfiles = launcher_runfiles.merge(ctx.runfiles(files = [ctx.file.compiler] + ([closure] if closure else [])))
    for target in ctx.attr.data:
        runfiles = runfiles.merge(target[DefaultInfo].default_runfiles).merge(ctx.runfiles(transitive_files = target[DefaultInfo].files))
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

dx_ldmd2 = rule(
    implementation = _dx_ldmd2_impl,
    doc = "Runs the pinned ldmd2 with the runtime closure directory as LD_LIBRARY_PATH.",
    executable = True,
    attrs = dict({
        "compiler": attr.label(allow_single_file = True, mandatory = True, doc = "The ldmd2 executable."),
        "closure_marker": attr.label(allow_single_file = True, doc = "A file in the runtime closure lib directory; unset when there is no closure."),
        "data": attr.label_list(allow_files = True, doc = "Files ldmd2 needs at run time."),
    }, **NATIVE_LAUNCHER_ATTRS),
)
