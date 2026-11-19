"""Analysis checks that Apple launchers run dx-runtime's apple_ops helper."""

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
