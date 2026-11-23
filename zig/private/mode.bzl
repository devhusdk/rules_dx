"""Whether a Zig target builds in ReleaseFast, from the mode or from the zigopts on its compile command line."""

ZigReleaseFastInfo = provider(doc = "Whether the Zig mode is ReleaseFast.", fields = {"value": "bool"})

def _zig_release_fast_impl(ctx):
    return [ZigReleaseFastInfo(value = ctx.attr.value)]

zig_release_fast = rule(
    implementation = _zig_release_fast_impl,
    doc = "Carries a select on the rules_zig mode, so a rule can read the mode without depending on the private flag.",
    attrs = {"value": attr.bool(mandatory = True)},
)

ZigOptsInfo = provider(doc = "Whether the Zig compile command line of a target sets ReleaseFast.", fields = {"release_fast": "bool"})

def _sets_release_fast(argv):
    """Whether argv holds -OReleaseFast or -O ReleaseFast."""
    for i, arg in enumerate(argv):
        if arg == "-OReleaseFast" or (arg == "-O" and argv[i + 1:i + 2] == ["ReleaseFast"]):
            return True
    return False

def _zig_opts_aspect_impl(target, _ctx):
    """Checks each Zig compile argv, which holds the expanded zigopts of the target and of every zig_library it imports."""
    release_fast = [a for a in target.actions if a.mnemonic.startswith("ZigBuild") and _sets_release_fast(a.argv)]
    return [ZigOptsInfo(release_fast = len(release_fast) > 0)]

zig_opts_aspect = aspect(
    implementation = _zig_opts_aspect_impl,
    doc = "Reads whether the Zig compile command line of a target sets ReleaseFast.",
)
