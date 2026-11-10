"""Analysis test for the swift-format copy action."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "SwiftFormatCopy"]
    asserts.equals(env, 1, len(actions), "SwiftFormatCopy actions")
    for action in actions:
        asserts.equals(env, "/dx_no_path", action.env.get("PATH", ""), "PATH")
        asserts.true(env, action.argv[0].startswith(("bazel-out/", "external/")), "program " + action.argv[0])
        asserts.equals(env, ["cp", "-L"], action.argv[1:3], "arguments")
    return analysistest.end(env)

swift_format_copy_test = analysistest.make(_impl)
