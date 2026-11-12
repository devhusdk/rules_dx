"""Windows SDK Build Tools acquisition behind an explicit license acceptance, and the msix module extension."""

load("//core:license.bzl", "accepted", "unaccepted")
load("//core:repo.bzl", "hub_repo", "pinned_archive")
load("//core:toolchain.bzl", "toolchain_target")
load(":pins.bzl", "BUILDTOOLS_BIN", "BUILDTOOLS_SHA256", "BUILDTOOLS_URL", "BUILDTOOLS_VERSION", "LICENSE", "LICENSE_URL", "TOOL_ARCHES")

HUB_NAME = "dx_msix_tools"

TOOLCHAINS_HUB_NAME = "dx_msix_toolchains"

_TOOLCHAIN_CPUS = {"arm64": "@platforms//cpu:arm64", "x64": "@platforms//cpu:x86_64"}

_TOOLCHAIN_NAMES = {"arm64": "windows_arm64", "x64": "windows_x86_64"}

UNACCEPTED = unaccepted("dx_msix", "Windows SDK Build Tools license", LICENSE_URL, LICENSE)

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

def hub_build(licensed):
    """Returns the BUILD text of the tool repository."""
    text = 'load("{}", "msix_toolchain")\n\npackage(default_visibility = ["//visibility:public"])\n'.format(str(Label(":toolchain.bzl")))
    for arch in TOOL_ARCHES:
        if licensed:
            text += _ACCEPTED_ARCH.format(arch = arch, bin = BUILDTOOLS_BIN, version = BUILDTOOLS_VERSION)
        else:
            text += _UNACCEPTED_ARCH.format(arch = arch, error = json.encode(UNACCEPTED))
    return text

def toolchains_hub_build():
    """Returns the BUILD text of the dx_msix_toolchains hub."""
    return "\n".join([
        toolchain_target(
            name = _TOOLCHAIN_NAMES[arch],
            exec_compatible_with = [_TOOLCHAIN_CPUS[arch], "@platforms//os:windows"],
            toolchain = "@{}//:{}".format(HUB_NAME, arch),
            toolchain_type = str(Label("//msix:toolchain_type")),
        )
        for arch in TOOL_ARCHES
    ])

def _msix_impl(mctx):
    if accepted(mctx, LICENSE):
        pinned_archive(
            name = HUB_NAME,
            files = {"BUILD.bazel": hub_build(True)},
            sha256 = BUILDTOOLS_SHA256,
            type = "zip",
            urls = [BUILDTOOLS_URL],
        )
    else:
        hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(False)})
    hub_repo(name = TOOLCHAINS_HUB_NAME, files = {"BUILD.bazel": toolchains_hub_build()})
    return mctx.extension_metadata(reproducible = True)

msix = module_extension(
    implementation = _msix_impl,
    doc = "Declares the pinned Windows SDK Build Tools repository and the dx_msix_toolchains hub.",
)
