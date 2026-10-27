"""Shared iOS library cell constants and helpers for apps, frameworks and libraries."""

MULTI_CPUS = {
    "device/arm64": "arm64",
    "simulator/arm64": "sim_arm64",
    "simulator/x86_64": "x86_64",
}

MACH_O_PLATFORMS = {
    "device": "2",
    "simulator": "7",
}

APPLE_HOSTS = select({
    Label("@platforms//os:ios"): [],
    Label("@platforms//os:macos"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

def has(ctx, constraint):
    """Returns whether the target platform has one constraint value."""
    return ctx.target_platform_has_constraint(constraint[platform_common.ConstraintValueInfo])

def target_cell(ctx):
    """Returns the environment/arch cell of the target platform, or empty off iOS."""
    if not has(ctx, ctx.attr._ios):
        return ""
    environment = "simulator" if has(ctx, ctx.attr._simulator) else "device"
    if has(ctx, ctx.attr._arm64):
        return environment + "/arm64"
    if has(ctx, ctx.attr._x86_64):
        return environment + "/x86_64"
    return environment + "/unknown"

def ios_platform(environment):
    """Returns the apple_common platform for one environment."""
    return apple_common.platform.ios_simulator if environment == "simulator" else apple_common.platform.ios_device

CELL_ATTRS = {
    "xcode_version": attr.string(default = ""),
    "_arm64": attr.label(default = "@platforms//cpu:arm64"),
    "_check": attr.label(
        default = Label("//apple/bundle:library_check.sh"),
        allow_single_file = True,
        cfg = "exec",
        executable = True,
    ),
    "_ios": attr.label(default = "@platforms//os:ios"),
    "_simulator": attr.label(default = "@apple_support//constraints:simulator"),
    "_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
    "_xcode_config": attr.label(
        default = configuration_field(
            name = "xcode_config_label",
            fragment = "apple",
        ),
    ),
}
