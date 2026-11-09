"""Builds Swift targets for the Swift Linux platform."""

load("//libs/starlark:platform.bzl", "single_platform_error")

_PLATFORMS = "//command_line_option:platforms"
_EXEC = "//command_line_option:extra_execution_platforms"
_TOOLCHAINS = "//command_line_option:extra_toolchains"

def swift_transition_settings(settings, attr):
    """Returns the settings Swift targets build with.

    Args:
        settings: The incoming command line option values.
        attr: The attributes of the rule being configured.

    Returns:
        The new platforms, execution platforms and toolchains.
    """
    toolchains = [str(t) for t in attr._swift_extra_toolchains]
    for t in settings[_TOOLCHAINS]:
        if str(t) not in toolchains:
            toolchains.append(str(t))
    if len(attr.platforms) == 0:
        return {_PLATFORMS: settings[_PLATFORMS], _EXEC: settings[_EXEC], _TOOLCHAINS: toolchains}
    error = single_platform_error("swift", attr.platforms)
    if error:
        fail(error)
    platform = str(attr.platforms[0])
    return {
        _PLATFORMS: [platform],
        _EXEC: [platform] + [str(p) for p in settings[_EXEC] if str(p) != platform],
        _TOOLCHAINS: toolchains,
    }

def _impl(settings, attr):
    return swift_transition_settings(settings, attr)

swift_linux_transition = transition(
    implementation = _impl,
    inputs = [_EXEC, _PLATFORMS, _TOOLCHAINS],
    outputs = [_PLATFORMS, _EXEC, _TOOLCHAINS],
)

def swift_linux_platforms():
    """Returns the Linux platform for the current target cpu, or no platform elsewhere."""
    return select({
        Label("//swift/platforms:is_linux_aarch64"): [Label("//swift/platforms:exec_linux_aarch64")],
        Label("//swift/platforms:is_linux_x86_64"): [Label("//swift/platforms:exec_linux_x86_64")],
        "//conditions:default": [],
    })
