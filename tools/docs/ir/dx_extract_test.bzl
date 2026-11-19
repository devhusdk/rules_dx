"""Analysis test that a docs extract action runs as a dx subcommand."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _dx_extract_test_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == ctx.attr.mnemonic]
    asserts.equals(env, 1, len(actions), "actions with mnemonic " + ctx.attr.mnemonic)
    if actions:
        argv = actions[0].argv
        asserts.true(env, argv[0].removesuffix(".exe").endswith("/dx"), "action runs " + argv[0])
        asserts.equals(env, ctx.attr.subcommand, argv[1])
    return analysistest.end(env)

dx_extract_test = analysistest.make(
    _dx_extract_test_impl,
    attrs = {
        "mnemonic": attr.string(mandatory = True),
        "subcommand": attr.string(mandatory = True),
    },
)
