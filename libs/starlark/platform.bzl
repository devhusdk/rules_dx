"""Reads the operating system and cpu of a Bazel platform label for rules that record them."""

DxPlatformInfo = provider(
    doc = "Operating system and cpu of the platform a probe was built for.",
    fields = {
        "cpu": "x86_64, aarch64, or empty when the cpu is not recognised.",
        "os": "linux, macos, windows, or empty when the os is not recognised.",
    },
)

def _platform_transition_impl(settings, attr):
    """Builds the probe for attr.platform, or for the current platform when it is unset."""
    if attr.platform == None:
        return {"//command_line_option:platforms": settings["//command_line_option:platforms"]}
    return {"//command_line_option:platforms": [str(attr.platform)]}

platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = ["//command_line_option:platforms"],
    outputs = ["//command_line_option:platforms"],
)

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
        default = Label("//libs/starlark:platform_probe"),
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
