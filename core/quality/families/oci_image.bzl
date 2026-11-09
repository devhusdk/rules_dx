"""OCI image real quality aspects."""

load("//deploy/private:image_providers.bzl", "OciImageQualityInfo")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

TOOLS = {
    "container_structure_test": {
        "attr": attr.label(default = "//config:tool_container_structure_test", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "oci_image",
    },
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
    "real_oci_image_typecheck": {
        "capability": "typecheck",
        "doc": "Additive OCI image typecheck family aspect (container-structure-test, tar driver).",
        "shard": "oci_image",
        "suffix": "-oci-image",
    },
}

def _trivy_image_config_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if OciImageQualityInfo not in target:
        return real_context()
    layout = target[OciImageQualityInfo].layout
    return real_context(inputs = [layout], options = {"args": [layout.path]})

def _container_structure_test_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if OciImageQualityInfo not in target:
        return real_context()
    info = target[OciImageQualityInfo]
    if not info.structure_tests:
        return real_context()
    return real_context(
        inputs = [info.layout] + info.structure_tests,
        options = {"args": [info.layout.path] + [test.path for test in info.structure_tests]},
    )

HOOK_ORDER = ["trivy_image_config", "container_structure_test"]

HOOKS = {
    "container_structure_test": _container_structure_test_hook,
    "trivy_image_config": _trivy_image_config_hook,
}

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

real_oci_image_typecheck_aspect = real_make_aspect(_CFG, "real_oci_image_typecheck")
