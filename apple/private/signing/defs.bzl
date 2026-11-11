"""Explicit macOS signing and notarization launchers that never run at build time."""

load("@rules_apple//apple:providers.bzl", "AppleBundleInfo")
load("//apple/private/bundle:defs.bzl", "display")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")
load(":validate.bzl", "macos_bundle_errors")

_SigningInfo = provider(
    doc = "The bundle id, archive and entitlements of one macOS app.",
    fields = {"bundle_id": "Bundle identifier.", "archive": "The built archive.", "entitlements": "The entitlements file or None."},
)

def _meta_impl(ctx):
    kind = ctx.attr.kind
    label = display(ctx.attr.app.label)
    if AppleBundleInfo not in ctx.attr.app:
        fail("{}: '{}' is not a macOS application".format(kind, label))
    info = ctx.attr.app[AppleBundleInfo]
    errors = macos_bundle_errors(kind, label, info.platform_type, info.bundle_extension, info.bundle_id)
    if errors:
        fail("\n".join(errors))
    files = [info.archive]
    entitlements = None
    if info.entitlements:
        files.append(info.entitlements)
        entitlements = info.entitlements
    return [
        DefaultInfo(runfiles = ctx.runfiles(files = files)),
        _SigningInfo(bundle_id = info.bundle_id, archive = info.archive, entitlements = entitlements),
    ]

_meta = rule(
    implementation = _meta_impl,
    doc = "Carries the bundle id, archive, and entitlements of one macOS app.",
    attrs = {
        "app": attr.label(doc = "The macOS application.", mandatory = True),
        "kind": attr.string(doc = "The macro name used in errors.", mandatory = True),
    },
)

def _launcher_impl(ctx):
    info = ctx.attr.meta[_SigningInfo]
    xcode = ctx.attr._xcode_config[apple_common.XcodeVersionConfig].xcode_version()
    lines = [
        "program=" + rlocation_path(ctx, ctx.executable._ops),
        "arg=" + ctx.attr.op,
        "arg=" + info.bundle_id,
    ]
    if ctx.attr.op == "codesign":
        lines.append("path=" + rlocation_path(ctx, info.archive))
        lines.append("path=" + rlocation_path(ctx, info.entitlements) if info.entitlements else "arg=")
    lines.append("env=DX_XCODE_VERSION\t" + (str(xcode) if xcode else ""))
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "\n".join(lines) + "\n")
    runfiles = [ctx.attr._ops[DefaultInfo].default_runfiles]
    if ctx.attr.op == "codesign":
        runfiles.append(ctx.attr.meta[DefaultInfo].default_runfiles)
    return DefaultInfo(executable = launcher, runfiles = launcher_runfiles.merge_all(runfiles))

_launcher_rule = rule(
    implementation = _launcher_impl,
    doc = "A launcher that signs or notarizes one macOS app.",
    attrs = {
        "meta": attr.label(doc = "The meta target of the app.", mandatory = True, providers = [_SigningInfo]),
        "op": attr.string(doc = "The operation: codesign or notarize.", mandatory = True, values = ["codesign", "notarize"]),
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

def _launcher(kind, op, name, app, visibility):
    _meta(
        name = name + "_meta",
        app = app,
        kind = kind,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _launcher_rule(
        name = name,
        meta = ":" + name + "_meta",
        op = op,
        tags = ["manual"],
        visibility = visibility,
    )

def apple_macos_sign(name, app, visibility = None):
    """Declares an explicit launcher that signs one macOS app with a Developer ID."""
    _launcher("apple_macos_sign", "codesign", name, app, visibility)

def apple_macos_notarize(name, app, visibility = None):
    """Declares an explicit launcher that notarizes and staples one signed macOS app."""
    _launcher("apple_macos_notarize", "notarize", name, app, visibility)
