"""Analysis checks of the Apple launchers and the xctestrun hook launchers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _descriptor_impl(ctx):
    env = analysistest.begin(ctx)
    lines = []
    for action in analysistest.target_actions(env):
        if any([output.basename.endswith(".launch") for output in action.outputs.to_list()]):
            lines = action.content.splitlines()
    asserts.true(env, len(lines) >= 3, str(lines))
    if len(lines) >= 3:
        asserts.true(env, lines[0].startswith("program=") and lines[0].removesuffix(".exe").endswith("cli/runtime/dx_runtime"), str(lines))
        asserts.equals(env, "env=DX_RUNTIME_TOOL\tapple_ops", lines[1])
    for line in ctx.attr.lines:
        asserts.true(env, line in lines, "no {} line in {}".format(line, lines))
    return analysistest.end(env)

apple_launcher_test = analysistest.make(
    _descriptor_impl,
    attrs = {"lines": attr.string_list(doc = "Lines the descriptor must hold.")},
)

def _hook_impl(ctx):
    env = analysistest.begin(ctx)
    lines = []
    for action in analysistest.target_actions(env):
        if any([output.basename.endswith(".launch") for output in action.outputs.to_list()]):
            lines = action.content.splitlines()
    asserts.equals(env, 1 + len(ctx.attr.prefixes), len(lines), str(lines))
    if len(lines) == 1 + len(ctx.attr.prefixes):
        asserts.true(env, lines[0].startswith("program=") and lines[0].removesuffix(".exe").endswith("/" + ctx.attr.program), str(lines))
        for prefix, line in zip(ctx.attr.prefixes, lines[1:]):
            asserts.true(env, line.startswith(prefix), "{} does not start with {}".format(line, prefix))
    return analysistest.end(env)

apple_hook_test = analysistest.make(
    _hook_impl,
    attrs = {
        "prefixes": attr.string_list(doc = "The prefix of each descriptor line after the program, in order."),
        "program": attr.string(default = "dx", doc = "Basename of the program the descriptor runs."),
    },
)
