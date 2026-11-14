"""Execution platforms and toolchains Swift targets build with."""

load("@platforms//host:constraints.bzl", "HOST_CONSTRAINTS")
load("//core:platform.bzl", "prepend_toolchains")

_EXEC = "//command_line_option:extra_execution_platforms"
_TOOLCHAINS = "//command_line_option:extra_toolchains"

def swift_exec_platform(platform, host_constraints):
    """Returns the execution platform that builds for a platform."""
    if "@platforms//os:linux" not in host_constraints:
        return platform
    cpu = "aarch64" if "@platforms//cpu:aarch64" in host_constraints else "x86_64"
    return str(Label("//swift/private/platforms:exec_linux_" + cpu))

def swift_platform_settings(settings, attr, platforms, host_constraints = HOST_CONSTRAINTS):
    """Returns the execution platforms and toolchains for Swift."""
    toolchains = prepend_toolchains(attr._swift_extra_toolchains, settings[_TOOLCHAINS])
    if len(platforms) == 0:
        return {_EXEC: settings[_EXEC], _TOOLCHAINS: toolchains}
    exec_platform = swift_exec_platform(platforms[0], host_constraints)
    return {
        _EXEC: [exec_platform] + [str(p) for p in settings[_EXEC] if str(p) != exec_platform],
        _TOOLCHAINS: toolchains,
    }

SWIFT_PLATFORM_SETTINGS = [_EXEC, _TOOLCHAINS]

def swift_linux_platforms():
    """Returns the Linux platform for the current target cpu, or no platform elsewhere."""
    return select({
        Label("//swift/private/platforms:is_linux_aarch64"): [str(Label("//swift/private/platforms:exec_linux_aarch64"))],
        Label("//swift/private/platforms:is_linux_x86_64"): [str(Label("//swift/private/platforms:exec_linux_x86_64"))],
        "//conditions:default": [],
    })
