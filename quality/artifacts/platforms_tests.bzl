"""Unit tests for canonical platform identities."""

load("//core:defs.bzl", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":extension.bzl", "TOOL_ARTIFACTS")
load(":hub.bzl", "ARTIFACT_PLATFORMS", "TOOL_PLATFORMS")
load(
    ":platforms.bzl",
    "artifact_platform_key",
    "execution_identity",
    "host_identity",
    "identity_error",
    "identity_key",
    "identity_role",
    "is_supported_key",
    "normalize_cpu",
    "normalize_os",
    "platform_key",
    "platform_key_error",
    "platform_repeat_message",
    "target_identity",
)

def platforms_tests(name):
    """Declares the canonical platform identity unit tests."""
    checks = [
        expect_equal("the supported execution set stays four platforms", TOOL_PLATFORMS, ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"]),
        expect_equal("canonical cpus normalize to themselves", [normalize_cpu("arm64"), normalize_cpu("x86_64")], ["arm64", "x86_64"]),
        expect_equal("canonical oses normalize to themselves", [normalize_os("linux"), normalize_os("macos"), normalize_os("windows")], ["linux", "macos", "windows"]),
        expect_equal("aliases and mixed case are rejected", [normalize_os("darwin"), normalize_os("win32"), normalize_os("Linux"), normalize_cpu("aarch64"), normalize_cpu("amd64"), normalize_cpu("x64"), normalize_cpu("AArch64")], ["", "", "", "", "", "", ""]),
        expect_equal("repeat platform tag names both modules", ["a 1" in platform_repeat_message("a 1", "b 2"), "b 2" in platform_repeat_message("a 1", "b 2")], [True, True]),
        expect_equal("unknown tokens normalize to empty", [normalize_os("solaris"), normalize_cpu("sparc"), normalize_os(""), normalize_cpu("")], ["", "", "", ""]),
        expect_equal("canonical pairs form canonical keys", [platform_key("linux", "x86_64"), platform_key("linux", "arm64"), platform_key("macos", "arm64"), platform_key("windows", "x86_64")], ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"]),
        expect_equal("alias pairs form no key", [platform_key("darwin", "arm64"), platform_key("Linux", "arm64"), platform_key("windows", "amd64")], ["", "", ""]),
        expect_equal("unknown pairs form no key", [platform_key("solaris", "sparc"), platform_key("linux", "sparc"), platform_key("solaris", "x86_64")], ["", "", ""]),
        expect_equal("wellformed but unclaimed pairs stay unsupported", [is_supported_key("macos_x86_64"), is_supported_key("windows_arm64")], [False, False]),
        expect_equal("known pairs need no diagnostic", [platform_key_error("linux", "arm64", "execution"), platform_key_error("windows", "x86_64", "target")], ["", ""]),
        expect_equal("unknown execution tokens fail explicitly", platform_key_error("solaris", "sparc", "execution"), "execution platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"),
        expect_equal("unknown host tokens fail explicitly", platform_key_error("plan9", "mips", "host"), "host platform: unknown os/cpu 'plan9/mips'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"),
        expect_equal("unknown target tokens fail explicitly", platform_key_error("fuchsia", "riscv64", "target"), "target platform: unknown os/cpu 'fuchsia/riscv64'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"),
        expect_equal("identities carry their roles", [identity_role(host_identity("linux", "x86_64")), identity_role(execution_identity("linux", "x86_64")), identity_role(target_identity("linux", "x86_64"))], ["host", "execution", "target"]),
        expect_equal("identities carry canonical keys", [identity_key(host_identity("linux", "arm64")), identity_key(execution_identity("macos", "arm64")), identity_key(target_identity("windows", "x86_64"))], ["linux_arm64", "macos_arm64", "windows_x86_64"]),
        expect_equal("known identities need no diagnostic", [identity_error(host_identity("linux", "x86_64")), identity_error(execution_identity("linux", "arm64")), identity_error(target_identity("windows", "x86_64"))], ["", "", ""]),
        expect_equal("unknown identities fail under their own role", [identity_error(host_identity("solaris", "sparc")), identity_error(execution_identity("solaris", "sparc")), identity_error(target_identity("solaris", "sparc"))], ["host platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)", "execution platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)", "target platform: unknown os/cpu 'solaris/sparc'; want os in (linux, macos, windows) and cpu in (x86_64, arm64)"]),
        expect_equal("a remote executor differs from the cli host", [identity_key(host_identity("linux", "x86_64")), identity_key(execution_identity("windows", "x86_64"))], ["linux_x86_64", "windows_x86_64"]),
        expect_equal("windows arm64 stays unclaimed", is_supported_key("windows_arm64"), False),
    ]
    for artifact in TOOL_ARTIFACTS:
        checks.append(expect_equal(
            artifact["tool"] + " " + artifact["os"] + "_" + artifact["cpu"] + " keys through the canonical mapping",
            artifact_platform_key(artifact),
            artifact["os"] + "_" + artifact["cpu"],
        ))
        key = artifact_platform_key(artifact)
        checks.append(expect_true(
            artifact["tool"] + " " + key + " is a selectable artifact platform",
            key in ARTIFACT_PLATFORMS,
        ))
        if key in TOOL_PLATFORMS:
            checks.append(expect_true(
                artifact["tool"] + " " + key + " is a supported execution key",
                is_supported_key(key),
            ))
    checks.append(expect_false("unexpected platform keys stay outside the supported set", is_supported_key("solaris_sparc")))
    starlark_test(
        name = name,
        mode = "unit",
        checks = checks,
    )
