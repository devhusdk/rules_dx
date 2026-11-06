"""Analysis test that ts_proto_srcs joins several descriptor sets into one flag."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "TsProto"]
    asserts.equals(env, 1, len(actions))
    argv = actions[0].argv
    flags = [i for i, arg in enumerate(argv) if arg == "--descriptor_set_in"]
    asserts.equals(env, 1, len(flags), "expected one --descriptor_set_in: %s" % argv)
    joined = argv[flags[0] + 1]
    asserts.true(env, len(joined.split(":")) >= 2, "descriptor sets not joined by ':': " + joined)
    return analysistest.end(env)

_test = analysistest.make(_impl)

def proto_args_tests(name, target):
    """Declares the descriptor set argv test.

    Args:
        name: Name of the test.
        target: ts_proto_srcs target to inspect.
    """
    _test(name = name, size = "small", target_under_test = target)
