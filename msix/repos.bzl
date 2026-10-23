"""Windows SDK Build Tools acquisition behind an explicit license acceptance."""

load(":pins.bzl", "BUILDTOOLS_BIN", "BUILDTOOLS_SHA256", "BUILDTOOLS_URL", "BUILDTOOLS_VERSION", "LICENSE", "LICENSE_URL", "TOOL_ARCHES")

HUB_NAME = "dx_msix_tools"

UNACCEPTED = "dx_msix: the Windows SDK Build Tools license (" + LICENSE_URL + ") is not accepted; add msix_tools.license(accept = [\"" + LICENSE + "\"]) to the root MODULE.bazel"

_ACCEPTED_ARCH = """
msix_toolchain(
    name = "{arch}",
    files = glob(["tools/bin/{bin}/{arch}/**"]),
    makeappx = "tools/bin/{bin}/{arch}/makeappx.exe",
    signtool = "tools/bin/{bin}/{arch}/signtool.exe",
    version = "{version}",
)

filegroup(
    name = "signtool_{arch}",
    srcs = glob(["tools/bin/{bin}/{arch}/**"]),
)
"""

_UNACCEPTED_ARCH = """
msix_toolchain(
    name = "{arch}",
    error = {error},
)

msix_unavailable(
    name = "signtool_{arch}",
    error = {error},
)
"""

_SIGNTOOL = """
alias(
    name = "signtool",
    actual = select({
        "@platforms//cpu:arm64": ":signtool_arm64",
        "//conditions:default": ":signtool_x64",
    }),
)
"""

def hub_build(accepted):
    """Returns the BUILD text of the tool repository."""
    text = 'load("{}", "msix_toolchain", "msix_unavailable")\n\npackage(default_visibility = ["//visibility:public"])\n'.format(str(Label(":toolchain.bzl")))
    for arch in TOOL_ARCHES:
        if accepted:
            text += _ACCEPTED_ARCH.format(arch = arch, bin = BUILDTOOLS_BIN, version = BUILDTOOLS_VERSION)
        else:
            text += _UNACCEPTED_ARCH.format(arch = arch, error = json.encode(UNACCEPTED))
    return text + _SIGNTOOL

def license_errors(licenses):
    """Returns one error per license name that rules_dx does not know."""
    return ["dx_msix: unknown license '" + name + "'; the only one is '" + LICENSE + "'" for name in licenses if name != LICENSE]

def _tools_impl(rctx):
    if rctx.attr.accepted:
        rctx.download_and_extract(
            url = [BUILDTOOLS_URL],
            output = "tools",
            sha256 = BUILDTOOLS_SHA256,
            type = "zip",
        )
    rctx.file("BUILD.bazel", hub_build(rctx.attr.accepted))

_tools = repository_rule(
    implementation = _tools_impl,
    doc = "Downloads the pinned Windows SDK Build Tools once the license is accepted.",
    attrs = {
        "accepted": attr.bool(doc = "Whether the root module accepted the license.", mandatory = True),
    },
)

def _extension_impl(mctx):
    licenses = []
    for module in mctx.modules:
        if not module.is_root:
            continue
        for tag in module.tags.license:
            licenses.extend(tag.accept)
    errors = license_errors(licenses)
    if errors:
        fail("; ".join(errors))
    _tools(name = HUB_NAME, accepted = LICENSE in licenses)
    return mctx.extension_metadata(reproducible = True)

_license = tag_class(
    doc = "Accepts the Windows SDK Build Tools license for this build.",
    attrs = {
        "accept": attr.string_list(doc = "License names to accept.", mandatory = True),
    },
)

msix_tools = module_extension(
    implementation = _extension_impl,
    doc = "Declares the pinned Windows SDK Build Tools repository.",
    tag_classes = {"license": _license},
)
