"""Analysis tests for Julia coverage on test forwarders."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:conformance.bzl", "dx_wrapper_contract_tests")
load("//core:defs.bzl", "expect_equal")
load("//core:entry.bzl", "dx_entry_quality_srcs")
load("//julia/private:defs.bzl", "JULIA_QUALITY_TOOLS", "JULIA_SOURCE_EXTS", "JULIA_SOURCE_SPECS")
load("//julia/private:platforms.bzl", "JULIA_PLATFORMS")

def _config_content(env):
    for action in analysistest.target_actions(env):
        for output in action.outputs.to_list():
            if output.basename.endswith(".julia_coverage.cfg"):
                return action.content
    return None

def _coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    content = _config_content(env)
    if ctx.attr.expect_runner:
        asserts.true(env, "DX_JULIA_COVERAGE_CONFIG" in run_env, "DX_JULIA_COVERAGE_CONFIG missing")
        asserts.true(env, content != None, "no coverage config written")
        if content != None:
            asserts.true(env, "source\t" + ctx.attr.source + "\n" in content, content)
            asserts.true(env, "upstream\t" + (target.label.repo_name or ctx.workspace_name) + "/" in content, content)
    else:
        asserts.false(env, "DX_JULIA_COVERAGE_CONFIG" in run_env, "coverage env set without coverage")
        asserts.equals(env, None, content)
    return analysistest.end(env)

_coverage_on_test = analysistest.make(
    _coverage_impl,
    attrs = {"expect_runner": attr.bool(default = True), "source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_off_test = analysistest.make(
    _coverage_impl,
    attrs = {"expect_runner": attr.bool(default = False), "source": attr.string()},
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def julia_coverage_test(name, target, source):
    """Asserts a Julia test gets the coverage config listing source under coverage, and only then."""
    _coverage_on_test(name = name, size = "small", target_under_test = target, source = source)
    _coverage_off_test(name = name + "_off", size = "small", target_under_test = target)

def _wrapper_action_impl(ctx):
    env = analysistest.begin(ctx)
    actions = analysistest.target_actions(env)
    mnemonics = [a.mnemonic for a in actions]
    asserts.true(env, "JuliaWrapper" in mnemonics, "no JuliaWrapper action, got: " + ", ".join(mnemonics))
    asserts.false(env, "TemplateExpand" in mnemonics, "wrapper template expanded at analysis time: " + ", ".join(mnemonics))
    return analysistest.end(env)

_wrapper_action_test = analysistest.make(_wrapper_action_impl)

def julia_wrapper_action_test(name, target):
    """Asserts the wrapper script is rendered by an action, not read at analysis time."""
    _wrapper_action_test(name = name, size = "small", target_under_test = target)

def julia_wrapper_contract_tests(name):
    """Declares the Julia wrapper contract tests."""
    dx_wrapper_contract_tests(
        name = name,
        extra_checks = [
            expect_equal(
                "wrappers carry julia_format",
                sorted(JULIA_QUALITY_TOOLS.keys()),
                ["julia_format"],
            ),
            expect_equal(
                "one julia class owns every source",
                JULIA_SOURCE_SPECS,
                [("julia", "jl")],
            ),
            expect_equal(
                "wrappers accept only Julia sources",
                JULIA_SOURCE_EXTS,
                [".jl"],
            ),
            expect_equal(
                "main leads the owned sources",
                dx_entry_quality_srcs(
                    "main.jl",
                    ["util.jl"],
                ),
                [
                    "main.jl",
                    "util.jl",
                ],
            ),
            expect_equal(
                "main listed in srcs is owned once",
                dx_entry_quality_srcs(
                    "main.jl",
                    [
                        "main.jl",
                        "util.jl",
                    ],
                ),
                [
                    "main.jl",
                    "util.jl",
                ],
            ),
            expect_equal(
                "no main and no srcs owns nothing",
                dx_entry_quality_srcs(None, None),
                [],
            ),
            expect_equal(
                "windows arm64 has no Julia build",
                [name for name, (os, cpu) in JULIA_PLATFORMS.items() if os == "windows" and cpu == "arm64"],
                [],
            ),
        ],
        file_classes = ["julia"],
    )
