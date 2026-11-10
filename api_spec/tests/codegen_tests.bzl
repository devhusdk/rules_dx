"""Analysis tests of the OpenAPI client actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _go_client_action_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "OpenapiGoClient"]
    asserts.equals(env, 1, len(actions))
    action = actions[0]
    asserts.equals(env, {"GOFLAGS": "-mod=mod", "GOPROXY": "off"}, action.env)
    argv = action.argv
    asserts.true(env, argv[0].removesuffix(".exe").endswith("/oapi_codegen"), "action runs " + argv[0])
    for arg in argv:
        asserts.false(env, arg.startswith("/"), "absolute argument " + arg)
        asserts.false(env, "$" in arg, "shell variable in argument " + arg)
    asserts.equals(env, ctx.attr.expected, argv[4:-1])
    return analysistest.end(env)

go_client_action_test = analysistest.make(
    _go_client_action_impl,
    attrs = {
        "expected": attr.string_list(mandatory = True, doc = "Generator arguments between the output and the root."),
    },
)
