"""Builds one target in the optimized release configuration."""

def _opt_transition_impl(_settings, _attr):
    """Selects the optimized compilation mode for one dependency edge."""
    return {"//command_line_option:compilation_mode": "opt"}

_opt_transition = transition(
    implementation = _opt_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:compilation_mode"],
)

def _release_artifact_impl(ctx):
    """Forwards the files and runfiles of the optimized target."""
    target = ctx.attr.target[0]
    return [DefaultInfo(
        files = target[DefaultInfo].files,
        runfiles = ctx.runfiles(transitive_files = target[DefaultInfo].files),
    )]

release_artifact = rule(
    implementation = _release_artifact_impl,
    doc = "Forwards one target built with compilation_mode=opt.",
    attrs = {
        "target": attr.label(
            doc = "The target to build in the release configuration.",
            mandatory = True,
            cfg = _opt_transition,
        ),
    },
)
