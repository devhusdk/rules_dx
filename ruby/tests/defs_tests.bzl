"""Wrapper conformance tests for Ruby."""

load("//core:conformance.bzl", "dx_wrapper_contract_tests")
load("//core:defs.bzl", "expect_equal")
load("//core:entry.bzl", "dx_infer_main", "dx_main_candidates")
load("//ruby/private/coverage:pins.bzl", "RUBY_COVERAGE_GEMS", "gem_dir")
load("//ruby/private:defs.bzl", "ruby_coverage_lib_dirs")

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
                "coverage pins simplecov 0.22.0 and its four companions",
                [gem_dir(g) for g in RUBY_COVERAGE_GEMS],
                ["simplecov-0.22.0", "simplecov-lcov-0.9.0", "docile-1.4.1", "simplecov-html-0.13.2", "simplecov_json_formatter-0.1.4"],
            ),
        ],
        **kwargs
    )

