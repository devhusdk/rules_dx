"""Wrapper conformance tests for Ruby."""

load("//core:conformance.bzl", "dx_wrapper_contract_tests")
load("//core:entry.bzl", "dx_infer_main", "dx_main_candidates")
load("//core/private/testing:defs.bzl", "expect_equal")
load("//ruby/private:defs.bzl", "ruby_coverage_lib_dirs")
load("//ruby/private/coverage:pins.bzl", "RUBY_COVERAGE_GEMS", "gem_dir")

_GEMS_ROOT = "../+ruby_coverage+dx_ruby_coverage_gems"

def _entries(root):
    return [root + "/" + gem_dir(g) + "/lib/" + g.entry for g in RUBY_COVERAGE_GEMS]

def ruby_wrapper_contract_tests(name, **kwargs):
    """Instantiates Ruby wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["ruby"],
        extra_checks = [
            expect_equal(
                "ruby_bench main defaults to the target name with .rb",
                dx_infer_main("ruby_bench", "sum_bench", None, ["helper.rb", "sum_bench.rb"], dx_main_candidates("sum_bench", ".rb")),
                "sum_bench.rb",
            ),
            expect_equal(
                "coverage lib dirs follow the pins",
                ruby_coverage_lib_dirs(["x/README.md"] + _entries(_GEMS_ROOT)),
                [_GEMS_ROOT + "/" + gem_dir(g) + "/lib" for g in RUBY_COVERAGE_GEMS],
            ),
            expect_equal(
                "coverage pins simplecov 1.3.2 and simplecov-lcov",
                [gem_dir(g) for g in RUBY_COVERAGE_GEMS],
                ["simplecov-1.3.2", "simplecov-lcov-0.9.0"],
            ),
        ],
        **kwargs
    )
