"""Analysis tests for the inputs and flags that make Erlang builds fail on bad code."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]

def _inputs(env, mnemonic):
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == mnemonic]
    asserts.equals(env, 1, len(actions), "expected one " + mnemonic + " action")
    return actions[0], [f.short_path for f in actions[0].inputs.to_list()]

def _warnings_as_errors_impl(ctx):
    env = analysistest.begin(ctx)
    action, _ = _inputs(env, "ERLC")
    asserts.true(env, "-Werror" in " ".join(action.argv), "ERLC does not fail on warnings")
    return analysistest.end(env)

def _has_impl(ctx):
    env = analysistest.begin(ctx)
    _, inputs = _inputs(env, "ERLC")
    found = [p for p in inputs if p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "ERLC inputs: " + str(inputs))
    return analysistest.end(env)

def _header_impl(ctx):
    env = analysistest.begin(ctx)
    headers = [f.short_path for f in analysistest.target_under_test(env)[CcInfo].compilation_context.headers.to_list()]
    found = [p for p in headers if p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "headers: " + str(headers))
    return analysistest.end(env)

def _define_impl(ctx):
    env = analysistest.begin(ctx)
    defines = analysistest.target_under_test(env)[CcInfo].compilation_context.defines.to_list()
    asserts.true(env, ctx.attr.define in defines, "defines: " + str(defines))
    return analysistest.end(env)

_define_test = analysistest.make(_define_impl, attrs = {"define": attr.string()})

_warnings_as_errors_test = analysistest.make(_warnings_as_errors_impl)
_has_attrs = {
    "file": attr.string(),
    "present": attr.bool(),
}
_has_test = analysistest.make(_has_impl, attrs = _has_attrs)
_header_test = analysistest.make(_header_impl, attrs = _has_attrs)

def erlc_warnings_as_errors_test(name, target):
    _warnings_as_errors_test(name = name, size = "small", target_under_test = target, target_compatible_with = _LINUX)

def erlc_input_test(name, target, file, present):
    _has_test(name = name, size = "small", target_under_test = target, file = file, present = present, target_compatible_with = _LINUX)

def nif_header_test(name, target, file, present):
    _header_test(name = name, size = "small", target_under_test = target, file = file, present = present, target_compatible_with = _LINUX)

def nif_define_test(name, target, define):
    _define_test(name = name, size = "small", target_under_test = target, define = define, target_compatible_with = _LINUX)
