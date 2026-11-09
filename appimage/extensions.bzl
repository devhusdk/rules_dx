"""The appimage_tools module extension."""

load(":repos.bzl", "HUB_NAME", "tools_repo")

def _extension_impl(mctx):
    tools_repo(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

appimage_tools = module_extension(
    implementation = _extension_impl,
    doc = "Declares the pinned AppImage tool repository.",
)
