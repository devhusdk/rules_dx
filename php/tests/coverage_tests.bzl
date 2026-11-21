"""Analysis tests of php_test under coverage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//php/private:providers.bzl", "PhpTestInfo")

_COVERAGE = {"//command_line_option:collect_code_coverage": True}

def _runfile_paths(env):
    return [f.short_path for f in analysistest.target_under_test(env)[DefaultInfo].default_runfiles.files.to_list()]

def _core_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, PhpTestInfo in analysistest.target_under_test(env), "the core php_test returns PhpTestInfo")
    asserts.equals(env, [], [path for path in _runfile_paths(env) if "pcov" in path])
    return analysistest.end(env)

php_core_coverage_test = analysistest.make(_core_impl, config_settings = _COVERAGE, doc = "The core php_test carries no PCOV under coverage.")

def _forward_impl(ctx):
    env = analysistest.begin(ctx)
    paths = _runfile_paths(env)
    name = analysistest.target_under_test(env).label.name
    asserts.true(env, [path for path in paths if path.endswith("/" + name + ".launch")] != [], "the forwarder writes its own descriptor")
    asserts.true(env, [path for path in paths if "pcov" in path] != [], "the forwarder ships PCOV")
    return analysistest.end(env)

php_forward_coverage_test = analysistest.make(_forward_impl, config_settings = _COVERAGE, doc = "The php_test forwarder runs PHPUnit under PCOV under coverage.")
