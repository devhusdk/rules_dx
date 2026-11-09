"""Windows SDK Build Tools acquisition behind an explicit license acceptance."""

load("@rules_dx//libs/starlark:license.bzl", "accepted", "license_tag", "unaccepted")
load("@rules_dx//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "BUILDTOOLS_BIN", "BUILDTOOLS_SHA256", "BUILDTOOLS_URL", "BUILDTOOLS_VERSION", "LICENSE", "LICENSE_URL", "TOOL_ARCHES")

HUB_NAME = "dx_msix_tools"

UNACCEPTED = unaccepted("dx_msix", "Windows SDK Build Tools license", LICENSE_URL, "msix_tools", LICENSE)

_ACCEPTED_ARCH = """
msix_toolchain(
    name = "{arch}",
    files = glob(["bin/{bin}/{arch}/**"]),
    makeappx = "bin/{bin}/{arch}/makeappx.exe",
    signtool = "bin/{bin}/{arch}/signtool.exe",
    version = "{version}",
)
"""

_UNACCEPTED_ARCH = """
msix_toolchain(
    name = "{arch}",
    error = {error},
)
"""

def hub_build(accepted):
    """Returns the BUILD text of the tool repository."""
    text = 'load("{}", "msix_toolchain")\n\npackage(default_visibility = ["//visibility:public"])\n'.format(str(Label(":toolchain.bzl")))
    for arch in TOOL_ARCHES:
        if accepted:
            text += _ACCEPTED_ARCH.format(arch = arch, bin = BUILDTOOLS_BIN, version = BUILDTOOLS_VERSION)
        else:
            text += _UNACCEPTED_ARCH.format(arch = arch, error = json.encode(UNACCEPTED))
    return text

def _extension_impl(mctx):
    if accepted(mctx, "dx_msix", LICENSE):
        pinned_archive(
            name = HUB_NAME,
            files = {"BUILD.bazel": hub_build(True)},
            sha256 = BUILDTOOLS_SHA256,
            type = "zip",
            urls = [BUILDTOOLS_URL],
        )
    else:
        hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(False)})
    return mctx.extension_metadata(reproducible = True)

_license = license_tag("Accepts the Windows SDK Build Tools license for this build.")

msix_tools = module_extension(
    implementation = _extension_impl,
    doc = "Declares the pinned Windows SDK Build Tools repository.",
    tag_classes = {"license": _license},
)
