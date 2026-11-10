"""Analysis tests shared by the per-language negative tests."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

LINUX_X86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == ctx.attr.mnemonic]
    asserts.equals(env, 1, len(actions), "expected one " + ctx.attr.mnemonic + " action")
    inputs = [f.short_path for f in actions[0].inputs.to_list()]
    found = [p for p in inputs if p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, ctx.attr.mnemonic + " inputs: " + str(inputs))
    return analysistest.end(env)

_action_input_test = analysistest.make(_impl, attrs = {
    "file": attr.string(),
    "mnemonic": attr.string(),
    "present": attr.bool(),
})

def action_input_test(name, target, mnemonic, file, present):
    """Checks whether the one mnemonic action of target reads file.

    Args:
      name: Test name.
      target: Target under test.
      mnemonic: Mnemonic of the action to inspect.
      file: Path suffix of the input.
      present: Whether the input must be present.
    """
    _action_input_test(
        name = name,
        size = "small",
        target_under_test = target,
        mnemonic = mnemonic,
        file = file,
        present = present,
        target_compatible_with = LINUX_X86_64,
    )

def platform_flip_transition(platform):
    """Returns a transition that builds its target for platform.

    Args:
      platform: Label string of the platform.

    Returns:
      A transition.
    """
    return transition(
        implementation = lambda _settings, _attr: {"//command_line_option:platforms": platform},
        inputs = [],
        outputs = ["//command_line_option:platforms"],
    )
