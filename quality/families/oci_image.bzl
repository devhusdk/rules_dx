"""OCI image real quality aspects."""

load("//deploy/private:image_providers.bzl", "OciImageQualityInfo")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

TOOLS = {
    "trivy_image_config": {
        "attr": attr.label(default = "//config:tool_trivy", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "oci_image",
    },
}

SHARD_SPECS = {
    "real_oci_image_lint": {
        "capability": "lint",
        "doc": "Additive OCI image lint family aspect (trivy image config).",
        "shard": "oci_image",
        "suffix": "-oci-image",
    },
}

def _trivy_image_config_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if OciImageQualityInfo not in target:
        return real_context()
    layout = target[OciImageQualityInfo].layout
    return real_context(inputs = [layout], options = {"args": [layout.path]})

HOOK_ORDER = ["trivy_image_config"]

HOOKS = {"trivy_image_config": _trivy_image_config_hook}

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [],
    harness_sources = lambda _target, _capability, direct_sources: direct_sources,
    toolchain = None,
    check_policy = False,
)

real_oci_image_lint_aspect = real_make_aspect(_CFG, "real_oci_image_lint")
