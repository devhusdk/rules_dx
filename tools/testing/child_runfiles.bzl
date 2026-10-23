"""Carries one binary with the runfiles manifest it reads as a child."""

def _child_runfiles_impl(ctx):
    """Returns the binary, its runfiles and its own runfiles manifest."""
    info = ctx.attr.binary[DefaultInfo]
    manifest = info.files_to_run.runfiles_manifest
    if manifest == None:
        fail("%s has no runfiles manifest" % ctx.attr.binary.label)
    files = [info.files_to_run.executable, manifest]
    return [DefaultInfo(
        files = depset(files),
        runfiles = ctx.runfiles(files = files).merge(info.default_runfiles),
    )]

child_runfiles = rule(
    implementation = _child_runfiles_impl,
    doc = "Carries one binary with the runfiles manifest it reads as a child.",
    attrs = {
        "binary": attr.label(
            cfg = "target",
            executable = True,
            mandatory = True,
            doc = "Binary a test runs as a child.",
        ),
    },
)
