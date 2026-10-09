"""Wasmtime acquisition pin coverage."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":repos.bzl", "ARTIFACTS", "PLATFORM_CONSTRAINTS", "WASMTIME_VERSION", "artifact_errors", "artifact_repositories", "no_match_error", "repository_name", "url_for")

_EXPECTED_CONSTRAINTS = {
    "linux_arm64": ["@platforms//os:linux", "@platforms//cpu:arm64"],
    "linux_x86_64": ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    "macos_arm64": ["@platforms//os:macos", "@platforms//cpu:arm64"],
    "macos_x86_64": ["@platforms//os:macos", "@platforms//cpu:x86_64"],
    "windows_x86_64": ["@platforms//os:windows", "@platforms//cpu:x86_64"],
}

def _wasmtime_checks():
    checks = []
    checks.append(expect_equal("wasmtime version", WASMTIME_VERSION, "49.0.2"))
    checks.append(expect_equal("wasmtime platforms", sorted(ARTIFACTS.keys()), [
        "linux_arm64",
        "linux_x86_64",
        "macos_arm64",
        "macos_x86_64",
        "windows_x86_64",
    ]))
    checks.append(expect_equal("wasmtime errors", artifact_errors(), []))
    checks.append(expect_equal(
        "wasmtime repositories",
        artifact_repositories(),
        {platform: "dx_wasmtime_" + platform for platform in sorted(ARTIFACTS.keys())},
    ))
    for platform in sorted(ARTIFACTS.keys()):
        artifact = ARTIFACTS[platform]
        checks.append(expect_equal(platform + " repository", repository_name(platform), "dx_wasmtime_" + platform))
        checks.append(expect_equal(platform + " sha256 length", len(artifact.sha256), 64))
        checks.append(expect_true(platform + " https url", url_for(artifact).startswith("https://")))
        checks.append(expect_true(platform + " versioned url", WASMTIME_VERSION in url_for(artifact)))
        checks.append(expect_equal(
            platform + " constraints",
            PLATFORM_CONSTRAINTS[platform],
            _EXPECTED_CONSTRAINTS[platform],
        ))
    checks.append(expect_true("wasmtime no-match names platforms", "windows_x86_64" in no_match_error()))
    return checks

def wasmtime_metadata_tests(name):
    """Declares the Wasmtime acquisition inventory pin test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _wasmtime_checks(),
    )
