"""Analysis tests for the repo-wide warnings-as-errors flag."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:wrapper.bzl", "dx_warnings_as_errors")

_FLAG = str(Label("//config:warnings_as_errors"))

_ProbeInfo = provider(doc = "The options a probe target received.", fields = ["opts"])

def _probe_impl(ctx):
    return [_ProbeInfo(opts = ctx.attr.opts)]

_probe = rule(
    implementation = _probe_impl,
    attrs = {"opts": attr.string_list()},
)

def _probe_macro(name, **kwargs):
    kwargs, opts = dx_warnings_as_errors(kwargs, ["-Werror"], ["-w"])
    _probe(name = name, opts = opts, tags = ["manual"], **kwargs)

def _opts_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, ctx.attr.expected, analysistest.target_under_test(env)[_ProbeInfo].opts)
    return analysistest.end(env)

def _make(flag):
    return analysistest.make(
        _opts_impl,
        attrs = {"expected": attr.string_list()},
        config_settings = {_FLAG: flag},
    )

_flag_on_test = _make(True)
_flag_off_test = _make(False)

def warnings_tests(name):
    """Declares the probes and the four flag and attribute combinations.

    Args:
      name: suite name and prefix of the declared targets.
    """
    _probe_macro(name = name + "_unset")
    _probe_macro(name = name + "_true", warnings_as_errors = True)
    _probe_macro(name = name + "_false", warnings_as_errors = False)
    _flag_on_test(name = name + "_flag_on", target_under_test = ":" + name + "_unset", expected = ["-Werror"])
    _flag_off_test(name = name + "_flag_off", target_under_test = ":" + name + "_unset", expected = ["-w"])
    _flag_off_test(name = name + "_attr_true_flag_off", target_under_test = ":" + name + "_true", expected = ["-Werror"])
    _flag_on_test(name = name + "_attr_false_flag_on", target_under_test = ":" + name + "_false", expected = ["-w"])
    native.test_suite(
        name = name,
        tests = [
            ":" + name + "_flag_on",
            ":" + name + "_flag_off",
            ":" + name + "_attr_true_flag_off",
            ":" + name + "_attr_false_flag_on",
        ],
    )
