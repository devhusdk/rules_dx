"""Ruby toolchain pin, host, line-ending and contract tests."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@dx_pins_data//:pins.bzl", "PINS")
load("@rules_ruby//ruby/private:download.bzl", "is_reproducible")
load("@rules_ruby//ruby/private:providers.bzl", "BundlerInfo")
load("//core:host.bzl", "host_cell_of")
load("//core/private/testing:defs.bzl", "expect_equal", "expect_true", "starlark_test")

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
    asserts.true(env, info.bundle_build_env != None, "rules_ruby toolchain has no bundle_build_env")
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

def ruby_toolchain_contract_tests(name, size):
    """Asserts the Ruby toolchain stays reproducible and keeps its bundle build options."""
    _probe(name = name + "_probe", tags = ["manual"])
    _test(
        name = name,
        target_under_test = ":" + name + "_probe",
        windows = select({"@platforms//os:windows": True, "//conditions:default": False}),
        size = size,
    )

def _frozen_impl(ctx):
    env = analysistest.begin(ctx)
    bundle_env = analysistest.target_under_test(env)[BundlerInfo].env
    asserts.equals(env, "true", bundle_env.get("BUNDLE_FROZEN"))
    return analysistest.end(env)

_frozen_test = analysistest.make(_frozen_impl)

def ruby_gems_frozen_tests(name, size):
    """Asserts the gem hub bundle runs with BUNDLE_FROZEN."""
    _frozen_test(
        name = name,
        target_under_test = "@gems//:gems",
        size = size,
    )

def _checks():
    return [
        expect_equal("linux host", host_cell_of("linux", "amd64"), "linux_x86_64"),
        expect_equal("mac arm host", host_cell_of("mac os x", "aarch64"), "macos_arm64"),
        expect_equal("windows host", host_cell_of("windows server 2022", "amd64"), "windows_x86_64"),
        expect_equal("sorbet pinned cells", sorted(PINS["dx_sorbet"]["cells"].keys()), ["linux_arm64", "linux_x86_64", "macos_arm64", "macos_x86_64", "windows_x86_64"]),
        expect_true("sorbet gems", all([c["type"] == "gem" for c in PINS["dx_sorbet"]["cells"].values() if "urls" in c])),
        expect_true("sorbet windows stub", "stub" in PINS["dx_sorbet"]["cells"]["windows_x86_64"]),
        expect_equal("reproducible pinned linux portable", _reproducible("3.4.12", platform = "x86_64_linux"), True),
        expect_equal("reproducible pinned windows portable", _reproducible("3.4.12", platform = "x86_64_windows", is_windows_target = True), True),
        expect_equal("reproducible host platform", _reproducible("3.4.12", platform = ""), False),
        expect_equal("reproducible unpinned", _reproducible("3.4.12", platform = "x86_64_linux", sha256 = None), False),
        expect_equal("reproducible system", _reproducible("system", platform = "x86_64_linux"), False),
        expect_equal("reproducible truffleruby", _reproducible("truffleruby-25.0.0", platform = "x86_64_linux"), False),
        expect_equal("reproducible pinned jruby", _reproducible("jruby-10.0.2.0", platform = "", portable_ruby = False), True),
    ]

def _reproducible(version, platform, is_windows_target = False, portable_ruby = True, sha256 = "0" * 64):
    return is_reproducible(
        version = version,
        portable_ruby = portable_ruby,
        is_windows_target = is_windows_target,
        platform = platform,
        sha256 = sha256,
    )

def ruby_metadata_tests(name, size):
    """Declares the Sorbet pin and Ruby repository reproducibility test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )

def ruby_crlf_tests(name, size):
    """Declares the test that the patched rules_ruby batch template keeps CRLF endings."""
    starlark_test(
        name = name,
        size = size,
        mode = "execution",
        file_line_endings = {"@rules_ruby//ruby/private/binary:binary.cmd.tpl": "crlf"},
        file_blocks = {
            "@rules_ruby//ruby/private/binary:binary.cmd.tpl": [
                'if "%COVERAGE%" == "1" (\r\n  call :rlocation {coverage_helper} _coverage_helper_path\r\n',
                "\r\n{bundler_command} {ruby_binary_name} !binary! {args} %*\r\n",
            ],
        },
    )
