"""Builds Swift targets for the Swift Linux platform."""

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
    toolchains = [str(t) for t in getattr(attr, "swift_extra_toolchains", [])]
    for t in settings[_TOOLCHAINS]:
        if str(t) not in toolchains:
            toolchains.append(str(t))
    platform = attr.swift_platform
    if platform == "":
        return {_PLATFORMS: settings[_PLATFORMS], _EXEC: settings[_EXEC], _TOOLCHAINS: toolchains}
    platform = str(Label(platform))
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

def swift_linux_platform():
    """Returns the Swift platform label for the current Linux cpu, or an empty string."""
    return select({
        Label("//swift/platforms:is_linux_aarch64"): "//swift/platforms:exec_linux_aarch64",
        Label("//swift/platforms:is_linux_x86_64"): "//swift/platforms:exec_linux_x86_64",
        "//conditions:default": "",
    })

def swift_extra_toolchains():
    """Returns the toolchains Swift targets prefer: the Xcode C++ toolchain on macOS."""
    return select({
        Label("@platforms//os:macos"): [str(Label("@local_config_apple_cc_toolchains//:all"))],
        "//conditions:default": [],
    })
