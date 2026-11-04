"""Builds Swift targets for the Swift Linux platform."""

_PLATFORMS = "//command_line_option:platforms"
_EXEC = "//command_line_option:extra_execution_platforms"

def _impl(settings, attr):
    platform = attr.swift_platform
    if platform == "":
        return {_PLATFORMS: settings[_PLATFORMS], _EXEC: settings[_EXEC]}
    platform = str(Label(platform))
    return {
        _PLATFORMS: [platform],
        _EXEC: [platform] + [str(p) for p in settings[_EXEC] if str(p) != platform],
    }

swift_linux_transition = transition(
    implementation = _impl,
    inputs = [_EXEC, _PLATFORMS],
    outputs = [_PLATFORMS, _EXEC],
)

def swift_linux_platform():
    """Returns the Swift platform label for the current Linux cpu, or an empty string."""
    return select({
        Label("//swift/platforms:is_linux_aarch64"): "//swift/platforms:exec_linux_aarch64",
        Label("//swift/platforms:is_linux_x86_64"): "//swift/platforms:exec_linux_x86_64",
        "//conditions:default": "",
    })
