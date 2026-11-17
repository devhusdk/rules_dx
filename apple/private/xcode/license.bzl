"""The Xcode license gate every Apple rule depends on."""

def _xcode_license_impl(ctx):
    if ctx.attr.error:
        fail(ctx.attr.error)
    return []

xcode_license = rule(
    implementation = _xcode_license_impl,
    doc = "Fails analysis with the unaccepted-license message when the root module has not accepted the Xcode license.",
    attrs = {
        "error": attr.string(doc = "Message analysis fails with. Empty when the license is accepted."),
    },
)

XCODE_LICENSE_ATTR = attr.label(
    default = Label("@dx_apple_xcode//:license"),
    doc = "The Xcode license gate.",
)
