"""Analysis tests for the platforms typescript_playwright_test accepts."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//typescript/private/playwright:repos.bzl", "ARTIFACTS", "playwright_compatible_with")

_ProbeInfo = provider(doc = "The constraints the Playwright compatibility select resolves to.", fields = ["constraints"])

def _probe_impl(ctx):
    return [_ProbeInfo(constraints = [str(constraint.label) for constraint in ctx.attr.constraints])]

_probe = rule(
    implementation = _probe_impl,
    attrs = {"constraints": attr.label_list()},
)

def _impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, ctx.attr.expected, analysistest.target_under_test(env)[_ProbeInfo].constraints)
    return analysistest.end(env)

def _make(platform):
    return analysistest.make(
        _impl,
        attrs = {"expected": attr.string_list()},
        config_settings = {"//command_line_option:platforms": [str(Label("//typescript/tests:" + platform))]},
    )

_linux_arm64_test = _make("linux_arm64")
_linux_x86_64_test = _make("linux_x86_64")
_macos_arm64_test = _make("macos_arm64")
_windows_arm64_test = _make("windows_arm64")
_windows_x86_64_test = _make("windows_x86_64")

_TESTS = {
    "linux_arm64": _linux_arm64_test,
    "linux_x86_64": _linux_x86_64_test,
    "macos_arm64": _macos_arm64_test,
    "windows_arm64": _windows_arm64_test,
    "windows_x86_64": _windows_x86_64_test,
}

_WINDOWS = {
    "windows_arm64": ["@platforms//os:windows", "@platforms//cpu:arm64"],
    "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
}

def playwright_compatibility_tests(name):
    """Checks that the compatibility select accepts the pinned platforms and rejects Windows."""
    _probe(name = name + "_probe", constraints = playwright_compatible_with())
    platforms = {platform: artifact.constraints for platform, artifact in ARTIFACTS.items()} | _WINDOWS
    if sorted(platforms.keys()) != sorted(_TESTS.keys()):
        fail("compatibility tests cover %s, want %s" % (sorted(_TESTS.keys()), sorted(platforms.keys())))
    tests = []
    for platform, constraints in platforms.items():
        native.platform(name = platform, constraint_values = constraints)
        _TESTS[platform](
            name = name + "_" + platform,
            expected = [] if platform in ARTIFACTS else [str(Label("@platforms//:incompatible"))],
            size = "small",
            target_under_test = ":" + name + "_probe",
        )
        tests.append(":" + name + "_" + platform)
    native.test_suite(name = name, tests = tests)
