"""Explicit macOS signing and notarization launchers that never run at build time."""

load("@rules_apple//apple:providers.bzl", "AppleBundleInfo")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("//apple/bundle:defs.bzl", "bundle_id_errors", "display")

def macos_bundle_errors(kind, label, platform_type, bundle_extension, bundle_id):
    """Returns the errors that keep one bundle from macOS signing."""
    if platform_type != "macos" or bundle_extension != ".app":
        return ["{}: '{}' is not a macOS application".format(kind, label)]
    return bundle_id_errors(kind, bundle_id)

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
    entitlements = ""
    if info.entitlements:
        files.append(info.entitlements)
        entitlements = info.entitlements.short_path
    meta = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(meta, "bundle_id={}\narchive={}\nentitlements={}\n".format(
        info.bundle_id,
        info.archive.short_path,
        entitlements,
    ))
    return [DefaultInfo(
        files = depset([meta]),
        runfiles = ctx.runfiles(files = [meta] + files),
    )]

_meta = rule(
    implementation = _meta_impl,
    doc = "Writes the bundle id, archive, and entitlements paths of one macOS app.",
    attrs = {
        "app": attr.label(doc = "The macOS application.", mandatory = True),
        "kind": attr.string(doc = "The macro name used in errors.", mandatory = True),
    },
)

def _launcher(kind, script, name, app, visibility):
    _meta(
        name = name + "_meta",
        app = app,
        kind = kind,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    sh_binary(
        name = name,
        srcs = [Label(script)],
        data = [":" + name + "_meta"],
        env = {"DX_SIGN_META": "$(rootpath :" + name + "_meta)"},
        tags = ["manual"],
        visibility = visibility,
    )

def apple_macos_sign(name, app, visibility = None):
    """Declares an explicit launcher that signs one macOS app with a Developer ID."""
    _launcher("apple_macos_sign", "//apple/signing:codesign_app.sh", name, app, visibility)

def apple_macos_notarize(name, app, visibility = None):
    """Declares an explicit launcher that notarizes and staples one signed macOS app."""
    _launcher("apple_macos_notarize", "//apple/signing:notarize_app.sh", name, app, visibility)
