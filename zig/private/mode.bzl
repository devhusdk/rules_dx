"""Whether a Zig target builds in ReleaseFast, from the mode or from its resolved zigopts."""

ZigReleaseFastInfo = provider(doc = "Whether the Zig mode is ReleaseFast.", fields = {"value": "bool"})

def _zig_release_fast_impl(ctx):
    return [ZigReleaseFastInfo(value = ctx.attr.value)]

zig_release_fast = rule(
    implementation = _zig_release_fast_impl,
    doc = "Carries a select on the rules_zig mode, so a rule can read the mode without depending on the private flag.",
    attrs = {"value": attr.bool(mandatory = True)},
)

ZigOptsInfo = provider(doc = "Whether the resolved zigopts of a Zig target set ReleaseFast.", fields = {"release_fast": "bool"})

def _sets_release_fast(opts):
    """Whether opts hold -OReleaseFast or -O ReleaseFast."""
    for i, opt in enumerate(opts):
        if opt == "-OReleaseFast" or (opt == "-O" and opts[i + 1:i + 2] == ["ReleaseFast"]):
            return True
    return False

def _zig_opts_aspect_impl(_target, ctx):
    return [ZigOptsInfo(release_fast = _sets_release_fast(getattr(ctx.rule.attr, "zigopts", None) or []))]

zig_opts_aspect = aspect(
    implementation = _zig_opts_aspect_impl,
    doc = "Reads the zigopts of a Zig target after their selects resolve.",
)
