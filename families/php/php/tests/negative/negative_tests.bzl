"""Analysis tests for the sources a PHP autoloader is built from."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _autoload_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "PhpAutoload"]
    asserts.equals(env, 1, len(actions), "expected one PhpAutoload action")
    paths = [f.short_path for f in actions[0].inputs.to_list()]
    found = [p for p in paths if p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "PhpAutoload inputs: " + str(paths))
    return analysistest.end(env)

_autoload_test = analysistest.make(_autoload_impl, attrs = {
    "file": attr.string(),
    "present": attr.bool(),
})

def php_autoload_input_test(name, target, file, present):
    """Checks whether the autoload action of target reads file."""
    _autoload_test(
        name = name,
        size = "small",
        target_under_test = target,
        file = file,
        present = present,
        target_compatible_with = _LINUX,
    )
