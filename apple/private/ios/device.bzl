"""Explicit device export launchers that never run at build time."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")
load(":app.bzl", "AppleIosDeviceAppInfo")

_ExportInfo = provider(
    doc = "The bundle id, IPA and profile of one device app.",
    fields = {"bundle_id": "Bundle identifier.", "ipa": "The built IPA.", "profile": "The provisioning profile."},
)

def _export_meta_impl(ctx):
    if AppleIosDeviceAppInfo not in ctx.attr.app:
        fail("apple_export: '{}' is not a device apple_ios_app".format(ctx.attr.app.label))
    info = ctx.attr.app[AppleIosDeviceAppInfo]
    return [
        DefaultInfo(runfiles = ctx.runfiles(files = [info.ipa, info.profile])),
        _ExportInfo(bundle_id = info.bundle_id, ipa = info.ipa, profile = info.profile),
    ]

_export_meta = rule(
    implementation = _export_meta_impl,
    doc = "Carries the bundle id, IPA, and profile of one device app.",
    attrs = {"app": attr.label(doc = "The device apple_ios_app.", mandatory = True)},
)

def _export_impl(ctx):
    info = ctx.attr.meta[_ExportInfo]
    xcode = ctx.attr._xcode_config[apple_common.XcodeVersionConfig].xcode_version()
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "\n".join([
        "program=" + rlocation_path(ctx, ctx.executable._ops),
        "arg=export",
        "arg=" + info.bundle_id,
        "path=" + rlocation_path(ctx, info.ipa),
        "path=" + rlocation_path(ctx, info.profile),
        "env=DX_XCODE_VERSION\t" + (str(xcode) if xcode else ""),
    ]) + "\n")
    return DefaultInfo(
        executable = launcher,
        runfiles = launcher_runfiles.merge_all([
            ctx.attr.meta[DefaultInfo].default_runfiles,
            ctx.attr._ops[DefaultInfo].default_runfiles,
        ]),
    )

_export = rule(
    implementation = _export_impl,
    doc = "A launcher that signs one device app into a distributable IPA.",
    attrs = {
        "meta": attr.label(doc = "The export_meta target of the app.", mandatory = True, providers = [_ExportInfo]),
        "_ops": attr.label(
            default = Label("//apple/private/tools/ops:apple_ops"),
            executable = True,
            cfg = "target",
        ),
        "_xcode_config": attr.label(
            default = configuration_field(name = "xcode_config_label", fragment = "apple"),
        ),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
    fragments = ["apple"],
)

def apple_ios_export(name, app, visibility = None):
    """Declares an explicit launcher that signs one device app into a distributable IPA."""
    _export_meta(
        name = name + "_meta",
        app = app,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _export(
        name = name,
        meta = ":" + name + "_meta",
        tags = ["manual"],
        visibility = visibility,
    )
