"""Wrapper conformance tests for Ruby."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load("//ruby/coverage:pins.bzl", "RUBY_COVERAGE_GEMS", "gem_dir")
load(":defs.bzl", "ruby_coverage_lib_dirs")

_GEMS_ROOT = "../+ruby_coverage+dx_ruby_coverage_gems"

def _entries(root):
    return [root + "/" + gem_dir(g) + "/lib/" + g.entry for g in RUBY_COVERAGE_GEMS]

def ruby_wrapper_contract_tests(name):
    """Instantiates Ruby wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["ruby"],
        extra_checks = [
            expect_equal(
                "coverage lib dirs follow the pins",
                ruby_coverage_lib_dirs(["x/README.md"] + _entries(_GEMS_ROOT)),
                [_GEMS_ROOT + "/" + gem_dir(g) + "/lib" for g in RUBY_COVERAGE_GEMS],
            ),
            expect_equal(
                "coverage pins simplecov 0.22.0 and its four companions",
                [gem_dir(g) for g in RUBY_COVERAGE_GEMS],
                ["simplecov-0.22.0", "simplecov-lcov-0.9.0", "docile-1.4.1", "simplecov-html-0.13.2", "simplecov_json_formatter-0.1.4"],
            ),
        ],
    )

def _coverage_env_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment if RunEnvironmentInfo in target else {}
    rubylib = run_env.get("RUBYLIB", "")
    runfiles = [f.short_path for f in target[DefaultInfo].default_runfiles.files.to_list()]
    if ctx.attr.coverage:
        for g in RUBY_COVERAGE_GEMS:
            want = "/" + gem_dir(g) + "/lib"
            asserts.true(env, want in rubylib, "RUBYLIB lacks " + want + ": " + rubylib)
        for entry in ["simplecov-0.22.0/lib/simplecov.rb", "simplecov-lcov-0.9.0/lib/simplecov-lcov.rb"]:
            asserts.true(env, [p for p in runfiles if p.endswith("/" + entry)] != [], "runfiles lack " + entry)

        # Test targets stay uninstrumented without --instrument_test_targets, as before #3954.
        asserts.true(env, InstrumentedFilesInfo in target, "no InstrumentedFilesInfo")
    else:
        asserts.equals(env, "", rubylib)
        asserts.equals(env, [], [p for p in runfiles if "dx_ruby_coverage_gems" in p])
    return analysistest.end(env)

_coverage_env_on_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"coverage": attr.bool(default = True)},
    config_settings = {"//command_line_option:collect_code_coverage": True},
)

_coverage_env_off_test = analysistest.make(
    _coverage_env_impl,
    attrs = {"coverage": attr.bool(default = False)},
    config_settings = {"//command_line_option:collect_code_coverage": False},
)

def ruby_coverage_env_test(name, target):
    """Asserts a Ruby test puts the pinned SimpleCov gems on RUBYLIB under coverage, and only then."""
    _coverage_env_on_test(name = name, size = "small", target_under_test = target)
    _coverage_env_off_test(name = name + "_off", size = "small", target_under_test = target)
