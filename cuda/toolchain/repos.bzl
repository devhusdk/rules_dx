"""CUDA toolkit license acceptance."""

HUB_NAME = "dx_cuda_license"

LICENSE = "cuda-eula"

LICENSE_URL = "https://docs.nvidia.com/cuda/eula/index.html"

UNACCEPTED = "dx_cuda: the CUDA Toolkit EULA (" + LICENSE_URL + ") is not accepted; add cuda_license.license(accept = [\"" + LICENSE + "\"]) to the root MODULE.bazel"

def hub_build(accepted):
    """Returns the BUILD text of the license repository."""
    return 'load("{}", "cuda_license")\n\npackage(default_visibility = ["//visibility:public"])\n\ncuda_license(\n    name = "license",\n    error = {},\n)\n'.format(
        str(Label(":check.bzl")),
        json.encode("" if accepted else UNACCEPTED),
    )

def license_errors(licenses):
    """Returns one error per license name that rules_dx does not know."""
    return ["dx_cuda: unknown license '" + name + "'; the only one is '" + LICENSE + "'" for name in licenses if name != LICENSE]

def _hub_impl(rctx):
    rctx.file("BUILD.bazel", hub_build(rctx.attr.accepted))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Records whether the root module accepted the CUDA Toolkit EULA.",
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
    _hub(name = HUB_NAME, accepted = LICENSE in licenses)
    return mctx.extension_metadata(reproducible = True)

_license = tag_class(
    doc = "Accepts the CUDA Toolkit EULA for this build.",
    attrs = {
        "accept": attr.string_list(doc = "License names to accept.", mandatory = True),
    },
)

cuda_license = module_extension(
    implementation = _extension_impl,
    doc = "Declares the CUDA Toolkit license repository.",
    tag_classes = {"license": _license},
)
