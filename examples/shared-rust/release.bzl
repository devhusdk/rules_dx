"""Builds one target in the optimized release configuration."""

def _opt_transition_impl(_settings, _attr):
    """Selects the optimized compilation mode for one dependency edge."""
    return {"//command_line_option:compilation_mode": "opt"}

_opt_transition = transition(
    implementation = _opt_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:compilation_mode"],
)

def _owner_prefix(file):
    """Returns the short path prefix of the package that owns one file."""
    owner = file.owner
    repo = ("../" + owner.workspace_name + "/") if owner.workspace_name else ""
    package = (owner.package + "/") if owner.package else ""
    return repo + package

def _release_artifact_impl(ctx):
    """Stages the optimized target's files under this target's own name."""
    staged = []
    for file in ctx.attr.target[0][DefaultInfo].files.to_list():
        prefix = _owner_prefix(file)
        if not file.short_path.startswith(prefix):
            fail("release_artifact " + str(ctx.label) + ": cannot place " + file.short_path)
        path = ctx.label.name + "/" + file.short_path[len(prefix):]
        if file.is_directory:
            out = ctx.actions.declare_directory(path)
        else:
            out = ctx.actions.declare_file(path)
        ctx.actions.symlink(output = out, target_file = file)
        staged.append(out)
    return [DefaultInfo(
        files = depset(staged),
        runfiles = ctx.runfiles(files = staged),
    )]

release_artifact = rule(
    implementation = _release_artifact_impl,
    doc = "Stages one target built with compilation_mode=opt under its own name.",
    attrs = {
        "target": attr.label(
            doc = "The target to build in the release configuration.",
            mandatory = True,
            cfg = _opt_transition,
        ),
    },
)
