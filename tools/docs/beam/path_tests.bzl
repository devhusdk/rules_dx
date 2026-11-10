"""Analysis test for the PATH of the Elixir and Erlang docs dump actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == ctx.attr.mnemonic]
    asserts.equals(env, 1, len(actions), ctx.attr.mnemonic + " actions")
    for action in actions:
        path = action.env.get("PATH", "")
        entries = path.split(":")
        asserts.true(env, path != "" and all([e.startswith(("bazel-out/", "external/")) for e in entries]), "PATH=" + path)
        asserts.true(env, action.argv[0].startswith(("bazel-out/", "external/")), "program " + action.argv[0])
    return analysistest.end(env)

_beam_shard_path_test = analysistest.make(
    _impl,
    attrs = {"mnemonic": attr.string(mandatory = True)},
)

def beam_shard_path_test(name, target, language, **kwargs):
    """Checks the dump action of one docs shard runs a declared program with only declared directories on PATH."""
    _beam_shard_path_test(
        name = name,
        target_under_test = target,
        mnemonic = "DxDocs" + language.capitalize(),
        **kwargs
    )
