"""Analysis tests for the module files that FortranCompile actions can see."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _has_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "FortranCompile"]
    actions = [a for a in actions if [f for f in a.inputs.to_list() if f.basename == ctx.attr.src]]
    asserts.equals(env, 1, len(actions), "expected one FortranCompile action for " + ctx.attr.src)
    inputs = [f.short_path for f in actions[0].inputs.to_list()]
    found = [p for p in inputs if ctx.attr.module_dir in p]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "FortranCompile inputs: " + str(inputs))
    return analysistest.end(env)

_has_test = analysistest.make(_has_impl, attrs = {
    "module_dir": attr.string(),
    "present": attr.bool(),
    "src": attr.string(),
})

def fortran_module_input_test(name, target, src, module_dir, present):
    """Checks whether the compile of src can see files under module_dir."""
    _has_test(
        name = name,
        size = "small",
        target_under_test = target,
        src = src,
        module_dir = module_dir,
        present = present,
        target_compatible_with = _LINUX,
    )
