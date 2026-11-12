"""Reads the operating system and cpu of a Bazel platform label for rules that record them."""

DxPlatformInfo = provider(
    doc = "Operating system and cpu of the platform a probe was built for.",
    fields = {
        "cpu": "x86_64, aarch64, or empty when the cpu is not recognised.",
        "os": "linux, macos, windows, or empty when the os is not recognised.",
    },
)

_PLATFORMS = "//command_line_option:platforms"

def _attr_platforms(attr):
    """Returns attr.platforms as strings, or [] when the rule has none."""
    return [str(p) for p in (getattr(attr, "platforms", None) or [])]

def _no_settings(_settings, _attr, _platforms):
    return {}

def _with_platforms(settings, platforms):
    out = dict(settings)
    out[_PLATFORMS] = platforms
    return out

def platforms_transition(implementation = _no_settings, inputs = [], outputs = [], platforms = _attr_platforms, single = ""):
    """Returns the transition that builds for a platforms list, one configuration per platform.

    Args:
        implementation: Called as implementation(settings, attr, platforms) with the one platform of a
            configuration, or [] for the command-line platform. Returns the other settings it changes.
        inputs: The settings implementation reads.
        outputs: The settings implementation returns.
        platforms: Called as platforms(attr). Returns the platform label strings. Empty keeps the
            command-line platform.
        single: The rule name. When set, more than one platform fails with single_platform_error.

    Returns:
        A transition that splits when platforms holds more than one label.
    """

    def _impl(settings, attr):
        chosen = platforms(attr)
        if not chosen:
            return _with_platforms(implementation(settings, attr, []), settings[_PLATFORMS])
        if single:
            error = single_platform_error(single, chosen)
            if error:
                fail(error)
        if len(chosen) == 1:
            return _with_platforms(implementation(settings, attr, chosen), chosen)
        return {p: _with_platforms(implementation(settings, attr, [p]), [p]) for p in chosen}

    return transition(
        implementation = _impl,
        inputs = [_PLATFORMS] + [s for s in inputs if s != _PLATFORMS],
        outputs = [_PLATFORMS] + [s for s in outputs if s != _PLATFORMS],
    )

platform_transition = platforms_transition(platforms = lambda attr: [str(attr.platform)] if attr.platform else [])

def _platform_probe_impl(ctx):
    """Reports the os and cpu of the target platform."""
    os = ""
    for name in ["linux", "macos", "windows"]:
        if ctx.target_platform_has_constraint(getattr(ctx.attr, "_os_" + name)[platform_common.ConstraintValueInfo]):
            os = name
    cpu = ""
    for name, constraint in [("x86_64", ctx.attr._cpu_x86_64), ("aarch64", ctx.attr._cpu_arm64)]:
        if ctx.target_platform_has_constraint(constraint[platform_common.ConstraintValueInfo]):
            cpu = name
    return [DxPlatformInfo(os = os, cpu = cpu)]

platform_probe = rule(
    implementation = _platform_probe_impl,
    doc = "Reports the os and cpu of the target platform as DxPlatformInfo.",
    attrs = {
        "_cpu_arm64": attr.label(default = Label("@platforms//cpu:arm64")),
        "_cpu_x86_64": attr.label(default = Label("@platforms//cpu:x86_64")),
        "_os_linux": attr.label(default = Label("@platforms//os:linux")),
        "_os_macos": attr.label(default = Label("@platforms//os:macos")),
        "_os_windows": attr.label(default = Label("@platforms//os:windows")),
    },
)

PLATFORM_ATTRS = {
    "platform": attr.label(
        doc = "Platform the recorded os and cpu describe. Defaults to the current target platform.",
        providers = [platform_common.PlatformInfo],
    ),
    "_platform_probe": attr.label(
        cfg = platform_transition,
        default = Label("//core:platform_probe"),
    ),
}

def platform_identity(ctx):
    """Returns the DxPlatformInfo for ctx.attr.platform, or for the current platform when unset."""
    return ctx.attr._platform_probe[0][DxPlatformInfo]

def single_platform_error(rule, platforms):
    """Returns the error for a platforms list that does not hold exactly one item, or ""."""
    if len(platforms) == 1:
        return ""
    return rule + ": platforms takes exactly one platform label, got " + str(len(platforms))

def prepend_toolchains(first, current):
    """Returns first followed by each toolchain of current that first does not already hold."""
    toolchains = [str(t) for t in first]
    for t in current:
        if str(t) not in toolchains:
            toolchains.append(str(t))
    return toolchains
