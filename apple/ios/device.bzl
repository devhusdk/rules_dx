"""Explicit device export launchers that never run at build time."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load(":app.bzl", "AppleIosDeviceAppInfo")

def _export_meta_impl(ctx):
    if AppleIosDeviceAppInfo not in ctx.attr.app:
        fail("apple_export: '{}' is not a device apple_ios_app".format(ctx.attr.app.label))
    info = ctx.attr.app[AppleIosDeviceAppInfo]
    meta = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(meta, "bundle_id={}\nipa={}\nprofile={}\n".format(
        info.bundle_id,
        info.ipa.short_path,
        info.profile.short_path,
    ))
    return [DefaultInfo(
        files = depset([meta]),
        runfiles = ctx.runfiles(files = [meta, info.ipa, info.profile]),
    )]

_export_meta = rule(
    implementation = _export_meta_impl,
    doc = "Writes the bundle id, IPA, and profile paths of one device app.",
    attrs = {"app": attr.label(doc = "The device apple_ios_app.", mandatory = True)},
)

def apple_ios_export(name, app, visibility = None):
    """Declares an explicit launcher that signs one device app into a distributable IPA."""
    _export_meta(
        name = name + "_meta",
        app = app,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    sh_binary(
        name = name,
        srcs = [Label("//apple/ios:export_ops.sh")],
        data = [":" + name + "_meta"],
        env = {"DX_EXPORT_META": "$(rootpath :" + name + "_meta)"},
        tags = ["manual"],
        visibility = visibility,
    )
