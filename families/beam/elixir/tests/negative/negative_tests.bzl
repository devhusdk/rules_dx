"""Analysis tests for what the Elixir compile action is given."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _compile_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "ELIXIRC"]
    asserts.equals(env, 1, len(actions), "expected one ELIXIRC action")
    action = actions[0]
    inputs = [f.short_path for f in action.inputs.to_list()]
    if ctx.attr.dep:
        found = [p for p in inputs if ctx.attr.dep in p]
        asserts.equals(env, ctx.attr.dep_present, len(found) > 0, "ELIXIRC inputs: " + str(inputs))
    if ctx.attr.flag:
        script = " ".join(action.argv)
        asserts.true(env, ctx.attr.flag in script, "ELIXIRC argv lacks " + ctx.attr.flag)
    return analysistest.end(env)

_compile_test = analysistest.make(_compile_impl, attrs = {
    "dep": attr.string(),
    "dep_present": attr.bool(),
    "flag": attr.string(),
})

def elixir_compile_test(name, target, dep = "", dep_present = False, flag = "", **kwargs):
    """Checks the flag and the dependency files of the compile of target."""
    _compile_test(
        name = name,
        size = "small",
        target_under_test = target,
        dep = dep,
        dep_present = dep_present,
        flag = flag,
        **kwargs
    )
