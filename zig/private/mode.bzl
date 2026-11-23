"""Whether the Zig mode of a configuration is ReleaseFast, for a wrapper to read after its upstream transition."""

ZigReleaseFastInfo = provider(doc = "Whether the Zig mode is ReleaseFast.", fields = {"value": "bool"})

def _zig_release_fast_impl(ctx):
    return [ZigReleaseFastInfo(value = ctx.attr.value)]

zig_release_fast = rule(
    implementation = _zig_release_fast_impl,
    doc = "Carries a select on the rules_zig mode, so a rule can read the mode without depending on the private flag.",
    attrs = {"value": attr.bool(mandatory = True)},
)
