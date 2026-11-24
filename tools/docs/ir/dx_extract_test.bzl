"""Analysis test that a docs action runs as a dx subcommand."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _is_dx(arg):
    return arg.removesuffix(".exe").endswith("/dx")

def _dx_extract_test_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == ctx.attr.mnemonic]
    asserts.equals(env, 1, len(actions), "actions with mnemonic " + ctx.attr.mnemonic)
    if actions:
        argv = actions[0].argv
        found = [i for i, arg in enumerate(argv) if _is_dx(arg)]
        asserts.true(env, len(found) > 0, "no dx in " + str(argv))
        if found:
            at = found[0] + 1 + ctx.attr.skip
            asserts.equals(env, ctx.attr.subcommand, argv[at] if at < len(argv) else None)
            asserts.true(env, ctx.attr.skip > 0 or found[0] == 0, "action runs " + argv[0])
    return analysistest.end(env)

dx_extract_test = analysistest.make(
    _dx_extract_test_impl,
    attrs = {
        "mnemonic": attr.string(mandatory = True, doc = "Mnemonic of the action under test."),
        "skip": attr.int(doc = "Arguments between dx and the subcommand."),
        "subcommand": attr.string(mandatory = True, doc = "dx subcommand the action runs."),
    },
)
