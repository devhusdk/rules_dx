"""The dx_apple_xcode repository that holds the Xcode license gate, and the apple module extension."""

load("//core:license.bzl", "APPLE_XCODE", "accepted", "unaccepted")
load("//core:repo.bzl", "hub_repo")

HUB_NAME = "dx_apple_xcode"

LICENSE_URL = "https://www.apple.com/legal/sla/docs/xcode.pdf"

UNACCEPTED = unaccepted("dx_apple", "Xcode and Apple SDKs Agreement", LICENSE_URL, APPLE_XCODE)

def hub_build(licensed):
    """Returns the BUILD text of the dx_apple_xcode repository."""
    return 'load("{}", "xcode_license")\n\nxcode_license(\n    name = "license",\n    error = {},\n    visibility = ["//visibility:public"],\n)\n'.format(
        str(Label(":license.bzl")),
        json.encode("" if licensed else UNACCEPTED),
    )

def _apple_impl(mctx):
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(accepted(mctx, APPLE_XCODE))})
    return mctx.extension_metadata(reproducible = True)

apple = module_extension(
    implementation = _apple_impl,
    doc = "Declares the dx_apple_xcode repository that gates every Apple rule on the Xcode license.",
)
