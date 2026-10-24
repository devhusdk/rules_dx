"""Builds one deployment for a chosen target platform."""

def _platform_transition_impl(_settings, attr):
    return {"//command_line_option:platforms": str(attr.platform)}

_platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _cross_deployment_impl(ctx):
    """Forwards the deployment files and runfiles built for the platform."""
    deployment = ctx.attr.deployment[0][DefaultInfo]
    return [DefaultInfo(
        files = depset(transitive = [deployment.files, deployment.default_runfiles.files]),
        runfiles = deployment.default_runfiles,
    )]

cross_deployment = rule(
    implementation = _cross_deployment_impl,
    doc = "Builds one deployment under a target platform.",
    attrs = {
        "deployment": attr.label(
            cfg = _platform_transition,
            mandatory = True,
            doc = "Deployment built for the platform.",
        ),
        "platform": attr.label(
            mandatory = True,
            doc = "Target platform for the deployment.",
        ),
    },
)
