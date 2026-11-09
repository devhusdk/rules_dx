"""Analysis tests for the inputs that make Dart builds fail on undeclared dependencies."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _module_inputs_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "DartModule"]
    asserts.equals(env, 1, len(actions), "expected one DartModule action")
    inputs = [f.short_path for f in actions[0].inputs.to_list()]
    found = [p for p in inputs if p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "DartModule inputs: " + str(inputs))
    return analysistest.end(env)

_module_inputs_test = analysistest.make(_module_inputs_impl, attrs = {
    "file": attr.string(),
    "present": attr.bool(),
})

def dart_module_input_test(name, target, file, present):
    _module_inputs_test(name = name, size = "small", target_under_test = target, file = file, present = present, target_compatible_with = _LINUX)
