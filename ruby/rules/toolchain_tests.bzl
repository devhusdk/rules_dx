"""Analysis tests for the rules_ruby toolchain contract (#3742)."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_TOOLCHAIN_TYPE = "@rules_ruby//ruby:toolchain_type"

ProbeInfo = provider(doc = "Toolchain env fields.", fields = ["env", "bundle_build_env"])

def _probe_impl(ctx):
    tc = ctx.toolchains[_TOOLCHAIN_TYPE]
    return [ProbeInfo(
        env = getattr(tc, "env", {}),
        bundle_build_env = getattr(tc, "bundle_build_env", None),
    )]

_probe = rule(
    implementation = _probe_impl,
    toolchains = [_TOOLCHAIN_TYPE],
)

def _absolute(value):
    return value.startswith("/") or (len(value) > 1 and value[1] == ":") or "=/" in value

def _impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ProbeInfo]
    asserts.true(env, info.bundle_build_env != None, "toolchain has no bundle_build_env; the reproducible patch is missing")
    bundle_env = info.bundle_build_env or {}
    for src in [info.env, bundle_env]:
        for k, v in src.items():
            asserts.false(env, _absolute(v), "absolute path in " + k + ": " + v)
    for k in info.env:
        asserts.false(env, k.startswith("BUNDLE_BUILD__"), "env carries " + k)
    if not ctx.attr.windows:
        asserts.true(env, bundle_env != {}, "bundle_build_env is empty off Windows")
        for k, v in bundle_env.items():
            want = {"BUNDLE_BUILD__PSYCH": "--with-libyaml-dir=dist", "BUNDLE_BUILD__OPENSSL": "--with-openssl-dir=dist"}.get(k)
            asserts.equals(env, want, v)
    return analysistest.end(env)

_test = analysistest.make(_impl, attrs = {"windows": attr.bool()})

def ruby_toolchain_contract_tests(name):
    """Asserts the Ruby toolchain stays reproducible and keeps its bundle build options.

    Args:
        name: test name.
    """
    _probe(name = name + "_probe", tags = ["manual"])
    _test(
        name = name,
        target_under_test = ":" + name + "_probe",
        windows = select({"@platforms//os:windows": True, "//conditions:default": False}),
        size = "small",
    )
