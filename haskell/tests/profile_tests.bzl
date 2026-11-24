"""Analysis tests that the ghcopts follow //config:config_tier and the warnings_as_errors attribute."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//haskell/private:defs.bzl", "DEFAULT_GHCOPTS", "STANDARD_GHCOPTS", "STRICT_GHCOPTS", "STRICT_LIBRARY_GHCOPTS")
load("//haskell/private:kwargs.bzl", "haskell_upstream_kwargs")

def _opts_impl(ctx):
    return [DefaultInfo(files = depset(), runfiles = ctx.runfiles()), _OptsInfo(opts = ctx.attr.opts)]

_OptsInfo = provider(fields = ["opts"])

_opts = rule(
    implementation = _opts_impl,
    attrs = {"opts": attr.string_list()},
)

def _opts_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, ctx.attr.expected, analysistest.target_under_test(env)[_OptsInfo].opts)
    return analysistest.end(env)

def _opts_test(tier):
    return analysistest.make(
        _opts_test_impl,
        attrs = {"expected": attr.string_list()},
        config_settings = {str(Label("//config:config_tier")): tier},
    )

_strict_test = _opts_test("strict")

_standard_test = _opts_test("standard")

_CASES = [
    ("default", DEFAULT_GHCOPTS, STRICT_GHCOPTS, STANDARD_GHCOPTS),
    (
        "werror_on",
        haskell_upstream_kwargs({"ghcopts": ["-O2"], "warnings_as_errors": True})["ghcopts"],
        STRICT_GHCOPTS + ["-O2", "-Werror"],
        STANDARD_GHCOPTS + ["-O2", "-Werror"],
    ),
    (
        "werror_off",
        haskell_upstream_kwargs({"ghcopts": ["-O2"], "warnings_as_errors": False})["ghcopts"],
        STRICT_GHCOPTS + ["-O2"],
        STANDARD_GHCOPTS + ["-O2"],
    ),
    (
        "library",
        haskell_upstream_kwargs({"warnings_as_errors": False}, library = True)["ghcopts"],
        STRICT_LIBRARY_GHCOPTS,
        STANDARD_GHCOPTS,
    ),
]

def profile_ghcopts_tests(name):
    """Declares tests that the ghcopts select on the tier and keep user flags and -Werror."""
    for case, opts, strict, standard in _CASES:
        target = "%s_%s_opts" % (name, case)
        _opts(name = target, opts = opts, tags = ["manual"])
        _strict_test(name = "%s_%s_strict" % (name, case), target_under_test = ":" + target, expected = strict)
        _standard_test(name = "%s_%s_standard" % (name, case), target_under_test = ":" + target, expected = standard)
