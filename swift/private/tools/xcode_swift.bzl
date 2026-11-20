"""The macOS `swift` command: a native launcher that runs an Xcode tool through dx-runtime."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:no_coverage.bzl", "no_coverage_transition")
load("//core:rlocation.bzl", "rlocation_path")

def _swift_xcode_launcher_impl(ctx):
    xcode = ctx.attr._xcode_config[apple_common.XcodeVersionConfig].xcode_version()
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "\n".join([
        "program=" + rlocation_path(ctx, ctx.executable._runtime),
        "env=DX_RUNTIME_TOOL\tapple_ops",
        "arg=" + ctx.attr.tool,
        "env=DX_XCODE_VERSION\t" + (str(xcode) if xcode else ""),
    ]) + "\n")
    return DefaultInfo(
        executable = launcher,
        runfiles = launcher_runfiles.merge(ctx.attr._runtime[0][DefaultInfo].default_runfiles),
    )

swift_xcode_launcher = rule(
    implementation = _swift_xcode_launcher_impl,
    doc = "A launcher that runs `tool` from the configured Xcode.",
    attrs = {
        "tool": attr.string(default = "swift", values = ["swift", "llvm-cov", "llvm-profdata"], doc = "The Xcode tool to run."),
        "_runtime": attr.label(
            default = Label("@dx_prebuilt//:runtime"),
            executable = True,
            cfg = no_coverage_transition,
            doc = "dx-runtime, whose apple_ops helper runs the operation.",
        ),
        "_xcode_config": attr.label(
            default = configuration_field(name = "xcode_config_label", fragment = "apple"),
        ),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
    fragments = ["apple"],
)
