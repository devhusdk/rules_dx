"""Tests that the JMH bench macros pass their attributes to the inner targets."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:defs.bzl", "expect_equal", "starlark_test")

def _compile_flag_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == ctx.attr.mnemonic]
    asserts.true(env, actions, "no %s action" % ctx.attr.mnemonic)
    for action in actions:
        asserts.true(env, ctx.attr.flag in action.argv, "%s is not an argument of %s" % (ctx.attr.flag, ctx.attr.mnemonic))
    return analysistest.end(env)

_compile_flag_test = analysistest.make(
    _compile_flag_impl,
    attrs = {
        "mnemonic": attr.string(mandatory = True),
        "flag": attr.string(mandatory = True),
    },
)

def jmh_compile_flag_test(name, bench, mnemonic, flag):
    """Asserts every `mnemonic` action that compiles the library of `bench` has `flag` as an argument."""
    _compile_flag_test(
        name = name,
        size = "small",
        target_under_test = ":" + bench + "_bin_upstream",
        mnemonic = mnemonic,
        flag = flag,
    )

def jmh_inner_attr_test(name, bench, inner, tags, target_compatible_with):
    """Asserts each `<bench><suffix>` in `inner`, declared earlier in this package, has every tag in `tags` and is compatible with `target_compatible_with` only."""
    want = [str(native.package_relative_label(label)) for label in target_compatible_with]
    checks = []
    for suffix in inner:
        rule = native.existing_rule(bench + suffix)
        if rule == None:
            fail("jmh_inner_attr_test: %s is not declared before this call" % (bench + suffix))
        checks.append(expect_equal(
            "%s%s tags" % (bench, suffix),
            [tag for tag in tags if tag in rule.get("tags", [])],
            tags,
        ))
        checks.append(expect_equal(
            "%s%s target_compatible_with" % (bench, suffix),
            [str(native.package_relative_label(label)) for label in rule.get("target_compatible_with", [])],
            want,
        ))
    starlark_test(name = name, mode = "unit", checks = checks)
