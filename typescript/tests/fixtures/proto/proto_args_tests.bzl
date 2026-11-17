"""Analysis test that ts_proto_srcs joins several descriptor sets into one flag."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cli/exec_os:defs.bzl", "ExecOsInfo")

def _impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "TsProto"]
    asserts.equals(env, 1, len(actions))
    argv = actions[0].argv
    flags = [i for i, arg in enumerate(argv) if arg == "--descriptor_set_in"]
    asserts.equals(env, 1, len(flags), "expected one --descriptor_set_in: %s" % argv)
    joined = argv[flags[0] + 1]

    # protoc splits --descriptor_set_in on the exec host's path list
    # separator, which is ';' on Windows and ':' elsewhere.
    sep = ";" if ctx.attr._exec_os[ExecOsInfo].windows else ":"
    asserts.true(env, len(joined.split(sep)) >= 2, "descriptor sets not joined by '%s': %s" % (sep, joined))
    return analysistest.end(env)

_test = analysistest.make(
    _impl,
    attrs = {
        "_exec_os": attr.label(
            default = "//cli/exec_os",
            cfg = "exec",
            providers = [ExecOsInfo],
        ),
    },
)

def proto_args_tests(name, target):
    """Declares the descriptor set argv test."""
    _test(name = name, size = "small", target_under_test = target)
