"""Unit test for the quality test configuration table."""

load("@bazel_skylib//lib:unittest.bzl", "asserts", "unittest")
load("//core/quality:consumer_policy_tests.bzl", "CONSUMER_POLICY_CONFIGS")
load("//core/quality/testdata/scalafix_aux:results.bzl", "SCALAFIX_CONFIG")
load("//core/quality/testdata/spotbugs_owned:results.bzl", "SPOTBUGS_CONFIG")
load(":real_result_cases.bzl", "REAL_RESULT_CASES")
load(":test_configs.bzl", "MAX_TEST_CONFIGS", "TEST_CONFIGS", "TEST_CONFIG_SETTINGS")
load(":tool_override_tests.bzl", "TOOL_OVERRIDE_CONFIGS")

def _table_impl(ctx):
    env = unittest.begin(ctx)
    names = sorted(TEST_CONFIGS.keys())
    asserts.true(env, len(names) <= MAX_TEST_CONFIGS, "%d entries, limit %d" % (len(names), MAX_TEST_CONFIGS))
    asserts.equals(env, [], [name for name in names if sorted(TEST_CONFIGS[name].keys()) != TEST_CONFIG_SETTINGS])
    equal = [
        a + " == " + b
        for i, a in enumerate(names)
        for b in names[i + 1:]
        if TEST_CONFIGS[a] == TEST_CONFIGS[b]
    ]
    asserts.equals(env, [], equal)
    used = {case["config"]: True for case in REAL_RESULT_CASES}
    for name in CONSUMER_POLICY_CONFIGS + TOOL_OVERRIDE_CONFIGS + [SCALAFIX_CONFIG, SPOTBUGS_CONFIG]:
        used[name] = True
    asserts.equals(env, names, sorted(used.keys()))
    return unittest.end(env)

_table_test = unittest.make(_table_impl)

def test_configs_test(name):
    """Declares the table test: entry limit, distinct entries, every entry used."""
    _table_test(name = name, size = "small")
