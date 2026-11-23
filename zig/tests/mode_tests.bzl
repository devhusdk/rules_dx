"""Analysis tests for the Zig optimize mode the wrappers build in."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_ZIG_MODE = str(Label("@rules_zig//zig/settings:mode"))

_ZigArgvInfo = provider(doc = "Zig compile command lines under one wrapper.", fields = {"argvs": "list of argv lists"})

def _zig_argv_aspect_impl(target, ctx):
    """Collects the Zig compile argv of the target and the wrapped targets under it."""
    argvs = [list(a.argv) for a in target.actions if a.mnemonic.startswith("ZigBuild")]
    for attr in ("upstream", "actual"):
        dep = getattr(ctx.rule.attr, attr, None)
        if type(dep) == "list":
            dep = dep[0]
        if dep and _ZigArgvInfo in dep:
            argvs.extend(dep[_ZigArgvInfo].argvs)
    return [_ZigArgvInfo(argvs = argvs)]

_zig_argv_aspect = aspect(
    implementation = _zig_argv_aspect_impl,
    attr_aspects = ["upstream", "actual"],
)

def _mode_impl(ctx):
    env = analysistest.begin(ctx)
    argvs = analysistest.target_under_test(env)[_ZigArgvInfo].argvs
    asserts.true(env, len(argvs) > 0, "expected a Zig compile action")
    for argv in argvs:
        modes = [argv[i + 1] for i in range(len(argv) - 1) if argv[i] == "-O"]
        asserts.equals(env, [ctx.attr.mode], modes, "zig argv: " + str(argv))
    return analysistest.end(env)

_mode_attrs = {"mode": attr.string(doc = "The one -O value every Zig compile action must carry.")}

_mode_test = analysistest.make(_mode_impl, attrs = _mode_attrs, extra_target_under_test_aspects = [_zig_argv_aspect])

_mode_opt_test = analysistest.make(
    _mode_impl,
    attrs = _mode_attrs,
    extra_target_under_test_aspects = [_zig_argv_aspect],
    config_settings = {"//command_line_option:compilation_mode": "opt"},
)

def _failure_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.message)
    return analysistest.end(env)

_failure_attrs = {"message": attr.string(doc = "Text the analysis failure must contain.")}

_release_fast_failure_test = analysistest.make(
    _failure_impl,
    attrs = _failure_attrs,
    expect_failure = True,
    config_settings = {_ZIG_MODE: "release_fast"},
)

_zigopts_failure_test = analysistest.make(_failure_impl, attrs = _failure_attrs, expect_failure = True)

def zig_mode_test(name, target, mode, opt = False):
    """Asserts that every Zig compile action under target passes `-O mode`, under -c opt when opt is set."""
    rule = _mode_opt_test if opt else _mode_test
    rule(name = name, size = "small", target_under_test = target, mode = mode)

def zig_release_fast_failure_test(name, target):
    """Asserts that target fails analysis under mode release_fast without allow_release_fast."""
    _release_fast_failure_test(name = name, size = "small", target_under_test = target, message = "Zig mode release_fast drops safety checks")

def zig_zigopts_failure_test(name, target):
    """Asserts that target fails analysis on ReleaseFast from its resolved zigopts without allow_release_fast."""
    _zigopts_failure_test(name = name, size = "small", target_under_test = target, message = "zigopts set ReleaseFast")
