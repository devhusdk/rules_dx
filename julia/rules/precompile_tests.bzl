"""Analysis test that Julia libraries precompile at build time."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":defs.bzl", "JULIA_COMPATIBLE_WITH")

def _precompile_impl(ctx):
    env = analysistest.begin(ctx)
    mnemonics = [a.mnemonic for a in analysistest.target_actions(env)]
    asserts.true(env, "JuliaPrecompile" in mnemonics, "no JuliaPrecompile action, got: " + ", ".join(mnemonics))
    return analysistest.end(env)

_precompile_test = analysistest.make(_precompile_impl)

def julia_precompile_test(name, target):
    """Asserts the upstream julia_library target runs a JuliaPrecompile action."""
    _precompile_test(
        name = name,
        size = "small",
        target_under_test = target,
        target_compatible_with = JULIA_COMPATIBLE_WITH,
    )
