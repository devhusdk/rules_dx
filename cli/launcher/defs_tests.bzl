"""Analysis checks of the launch descriptors cli/launcher/defs.bzl writes."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _subcommand_descriptor_impl(ctx):
    env = analysistest.begin(ctx)
    lines = []
    for action in analysistest.target_actions(env):
        if any([output.basename.endswith(".launch") for output in action.outputs.to_list()]):
            lines = action.content.splitlines()
    asserts.equals(env, 2, len(lines), str(lines))
    if len(lines) == 2:
        asserts.true(env, lines[0].startswith("program=") and lines[0].removesuffix(".exe").endswith("/dx"), str(lines))
        asserts.equals(env, "arg=" + ctx.attr.subcommand, lines[1])
    return analysistest.end(env)

dx_subcommand_test = analysistest.make(
    _subcommand_descriptor_impl,
    attrs = {"subcommand": attr.string(doc = "The subcommand the descriptor must pass.")},
)
