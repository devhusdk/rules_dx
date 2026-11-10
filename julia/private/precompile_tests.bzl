"""Analysis tests that Julia libraries precompile and forward JuliaInfo."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":defs.bzl", "JULIA_COMPATIBLE_WITH", "JuliaInfo")

def _precompile_impl(ctx):
    env = analysistest.begin(ctx)
    actions = analysistest.target_actions(env)
    mnemonics = [a.mnemonic for a in actions]
    asserts.true(env, "JuliaPrecompile" in mnemonics, "no JuliaPrecompile action, got: " + ", ".join(mnemonics))
    if ctx.attr.package:
        precompiles = [a for a in actions if a.mnemonic == "JuliaPrecompile"]
        asserts.equals(env, 1, len(precompiles))
        if precompiles:
            argv = precompiles[0].argv
            asserts.true(env, "--package" in argv, "no --package in " + str(argv))
            if "--package" in argv:
                asserts.equals(env, ctx.attr.package, argv[argv.index("--package") + 1])
    return analysistest.end(env)

_precompile_test = analysistest.make(
    _precompile_impl,
    attrs = {"package": attr.string()},
)

def julia_precompile_test(name, target, package = ""):
    """Asserts the upstream julia_library target runs a JuliaPrecompile action.

    Args:
        name: Test name.
        target: Label of the upstream julia_library.
        package: Optional module name the single precompile action must pass as --package.
    """
    _precompile_test(
        name = name,
        size = "small",
        target_under_test = target,
        package = package,
        target_compatible_with = JULIA_COMPATIBLE_WITH,
    )

def _forwarded_info_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[JuliaInfo]
    asserts.equals(env, ctx.attr.package, info.app_name)
    asserts.true(env, bool(info.entry), "empty entry")
    return analysistest.end(env)

_forwarded_info_test = analysistest.make(
    _forwarded_info_impl,
    attrs = {"package": attr.string()},
)

def julia_forwarded_info_test(name, target, package):
    """Asserts the dx julia_library forwards JuliaInfo with the module name.

    Args:
        name: Test name.
        target: Label of the dx julia_library.
        package: Expected module name (JuliaInfo.app_name).
    """
    _forwarded_info_test(
        name = name,
        size = "small",
        target_under_test = target,
        package = package,
        target_compatible_with = JULIA_COMPATIBLE_WITH,
    )
