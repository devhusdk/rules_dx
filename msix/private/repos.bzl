"""Windows SDK Build Tools acquisition behind an explicit license acceptance."""

load("//core:license.bzl", "unaccepted")
load(":pins.bzl", "BUILDTOOLS_BIN", "BUILDTOOLS_VERSION", "LICENSE", "LICENSE_URL", "TOOL_ARCHES")

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
