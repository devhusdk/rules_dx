"""Test-only wrapper that sets rules_android's Starlark android_platforms flag around a target."""

_STARLARK_PLATFORMS = "@rules_android//flags:android_platforms"

def _starlark_platforms_transition_impl(_settings, attr):
    return {_STARLARK_PLATFORMS: [str(platform) for platform in attr.platforms]}

_starlark_platforms_transition = transition(
    implementation = _starlark_platforms_transition_impl,
    inputs = [],
    outputs = [_STARLARK_PLATFORMS],
)

def _starlark_platforms_impl(ctx):
    return [DefaultInfo(files = ctx.attr.src[0][DefaultInfo].files)]

starlark_platforms = rule(
    implementation = _starlark_platforms_impl,
    doc = "Forwards src's files built with @rules_android//flags:android_platforms set to platforms.",
    attrs = {
        "platforms": attr.label_list(mandatory = True, doc = "Value for the Starlark flag."),
        "src": attr.label(mandatory = True, cfg = _starlark_platforms_transition, doc = "Target to rebuild."),
    },
)
