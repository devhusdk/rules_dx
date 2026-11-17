"""Analysis proofs for the consumer-selected workspace policy."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cc/private:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")
load(
    "//core/quality/testdata:all_languages.bzl",
    "real_python_lint_aspect",
    "real_python_typecheck_aspect",
    "real_typecheck_probe_aspect",
    _ASPECTS_BY_RESULT = "REAL_ASPECTS_BY_RESULT",
)
load("//core/quality/testdata:config_subject.bzl", "config_attr", "config_subject_rule", "config_transition")
load("//core/quality:sources.bzl", "single_target")

CONSUMER_POLICY_CONFIGS = ["consumer_cc_clang_tidy", "consumer_format_off", "default", "generated", "shared"]

POLICY_CONFIG_TEST_CONFIGS = ["consumer_python_ruff", "default_standard", "strict_generated_standard"]

def _dx_result_names(target):
    """Returns the sorted real-aspect result names published for one target."""
    if OutputGroupInfo not in target:
        return []
    groups = target[OutputGroupInfo]
    if "dx_results" not in groups:
        return []
    return sorted([f.basename for f in groups["dx_results"].to_list()])

def _subject_attr(key):
    """Returns the subject attribute name carrying one aspect key."""
    return "subject_" + key.replace("-", "_")

def _consumer_policy_impl(ctx):
    results = {}
    subjects = []
    for key in _ASPECTS_BY_RESULT:
        dep = getattr(ctx.attr, _subject_attr(key))
        if not dep:
            continue
        subject = single_target(dep)
        subjects.append(str(subject.label))
        for name in _dx_result_names(subject):
            results[name] = True
    results = sorted(results)
    if results != ctx.attr.expected_dx_results:
        fail("consumer policy config " + ctx.attr.config + " publishes " +
             str(results) + " real result(s) for " + str(subjects) +
             ", want " + str(ctx.attr.expected_dx_results))
    windows = ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])
    out = ctx.actions.declare_file(ctx.label.name + (".bat" if windows else ".sh"))
    ctx.actions.write(
        output = out,
        content = "@exit /b 0\r\n" if windows else "#!/bin/sh\nexit 0\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out)]

_consumer_policy_test = rule(
    implementation = _consumer_policy_impl,
    test = True,
    attrs = dict(
        {
            "expected_dx_results": attr.string_list(
                mandatory = True,
            ),
            "config": config_attr(),
            "_windows_os": attr.label(
                default = "@platforms//os:windows",
            ),
        },
        **{
            _subject_attr(key): attr.label(
                aspects = [aspect],
                cfg = config_transition,
            )
            for key, aspect in _ASPECTS_BY_RESULT.items()
        }
    ),
)

def _result_keys(expected_dx_results, absent_keys):
    """Returns the sorted aspect keys named by expected results plus the keys that must stay absent."""
    keys = {}
    for key in absent_keys:
        if key not in _ASPECTS_BY_RESULT:
            fail("no real aspect for absent key " + key)
        keys[key] = True
    for result in expected_dx_results:
        key = result.rsplit("-real-", 1)[1].removesuffix(".pb")
        if key not in _ASPECTS_BY_RESULT:
            fail("no real aspect for expected result " + result)
        keys[key] = True
    return sorted(keys) or ["lint-text"]

def _ruff_config_inputs(env):
    inputs = analysistest.target_under_test(env)[OutputGroupInfo].dx_inputs.to_list()
    return sorted([f.short_path for f in inputs if f.basename == "ruff.toml"])

_ruff_subject = config_subject_rule([real_python_lint_aspect], [OutputGroupInfo])

def _policy_config_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, [ctx.attr.expected_config], _ruff_config_inputs(env))
    return analysistest.end(env)

_policy_config_test = analysistest.make(
    _policy_config_impl,
    attrs = {"expected_config": attr.string(mandatory = True)},
)

_tidy_subject = config_subject_rule([clang_syntax_aspect], [ClangSyntaxInfo])

def _policy_clang_tidy_impl(ctx):
    env = analysistest.begin(ctx)
    tidy_inputs = []
    for inputs in analysistest.target_under_test(env)[ClangSyntaxInfo].tidy_inputs:
        tidy_inputs += [f.basename for f in inputs.to_list()]
    asserts.true(env, ".clang-tidy" in tidy_inputs, "want .clang-tidy in the clang-tidy inputs, got " + str(tidy_inputs))
    return analysistest.end(env)

_policy_clang_tidy_test = analysistest.make(
    _policy_clang_tidy_impl,
)

def _policy_generated_impl(ctx):
    env = analysistest.begin(ctx)
    return analysistest.end(env)

_policy_generated_test = analysistest.make(
    _policy_generated_impl,
)

_generated_subject = config_subject_rule([real_typecheck_probe_aspect])

def _tier_base_inputs(env):
    paths = {}
    for f in analysistest.target_under_test(env)[OutputGroupInfo].dx_inputs.to_list():
        if f.short_path.startswith("core/quality/tiers/ruff/") or f.short_path.startswith("core/quality/tiers/ty/"):
            paths[f.short_path] = True
    return sorted(paths.keys())

def _tier_base_impl(ctx):
    env = analysistest.begin(ctx)
    profile = ctx.attr.expected_profile
    want = ["core/quality/tiers/ruff/%s.toml" % profile, "core/quality/tiers/ty/%s.toml" % profile]
    asserts.equals(env, want, _tier_base_inputs(env))
    return analysistest.end(env)

_TIER_BASE_ATTRS = {"expected_profile": attr.string(mandatory = True)}
_tier_base_subject = config_subject_rule([real_python_lint_aspect, real_python_typecheck_aspect], [OutputGroupInfo])
_tier_base_test = analysistest.make(_tier_base_impl, attrs = _TIER_BASE_ATTRS)

def policy_config_tests(name):
    """Proves policy configs bind a target, aspect_hints override them, tier bases follow the profile and a rendered policy analyzes."""
    pkg = "core/quality/tests/fixtures/consumer_policy/"
    fixtures = "//core/quality/tests/fixtures/consumer_policy:"
    for suffix, target, expected in [
        ("_policy", "policy_bound_py", pkg + "ruff.toml"),
        ("_hint", "hinted_py", pkg + "override/ruff.toml"),
    ]:
        _ruff_subject(
            name = name + suffix + "_subject",
            config = "consumer_python_ruff",
            tags = ["manual"],
            target = fixtures + target,
        )
        _policy_config_test(
            name = name + suffix,
            size = "small",
            expected_config = expected,
            target_under_test = name + suffix + "_subject",
        )
    _tidy_subject(
        name = name + "_clang_tidy_subject",
        config = "consumer_cc_clang_tidy",
        tags = ["manual"],
        target = fixtures + "policy_bound_c",
    )
    _policy_clang_tidy_test(
        name = name + "_clang_tidy",
        size = "small",
        target_under_test = name + "_clang_tidy_subject",
    )
    _generated_subject(
        name = name + "_generated_subject",
        config = "generated",
        tags = ["manual"],
        target = fixtures + "policy_bound_py",
    )
    _policy_generated_test(
        name = name + "_generated",
        size = "small",
        target_under_test = name + "_generated_subject",
    )
    for suffix, config, profile in [
        ("_tier_base_default", "default_standard", "standard"),
        ("_tier_base_default_strict", "default", "strict"),
        ("_tier_base_generated_strict", "strict_generated_standard", "strict"),
    ]:
        _tier_base_subject(
            name = name + suffix + "_subject",
            config = config,
            tags = ["manual"],
            target = fixtures + "policy_bound_py",
        )
        _tier_base_test(
            name = name + suffix,
            size = "small",
            expected_profile = profile,
            target_under_test = name + suffix + "_subject",
        )
    native.test_suite(name = name, tests = [
        name + "_policy",
        name + "_hint",
        name + "_clang_tidy",
        name + "_generated",
        name + "_tier_base_default",
        name + "_tier_base_default_strict",
        name + "_tier_base_generated_strict",
    ])

def consumer_policy_tests(name, cases, **kwargs):
    """Instantiates one proof per consumer policy selection."""
    kwargs.setdefault("size", "small")
    configs = sorted({case["config"]: True for case in cases}.keys())
    if configs != CONSUMER_POLICY_CONFIGS:
        fail("%s: cases use configs %s, want %s" % (name, configs, CONSUMER_POLICY_CONFIGS))
    for case in cases:
        subjects = {_subject_attr(key): case["subject"] for key in _result_keys(case["expected_dx_results"], case.get("absent_keys", []))}
        _consumer_policy_test(
            name = name + "_" + case["case"] + "_test",
            expected_dx_results = case["expected_dx_results"],
            config = case["config"],
            **dict(kwargs, **subjects)
        )
