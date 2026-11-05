"""Terraform real quality aspects."""

load("//quality:family_hooks.bzl", "default_files_hook")
load("//quality:pipeline.bzl", "real_request_tool_env")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//terraform/rules:providers.bzl", "TerraformModuleInfo")

TOOLS = {
    "terraform": {
        "attr": attr.label(default = "//config:tool_terraform", allow_single_file = True, cfg = "exec"),
        "env": [["CHECKPOINT_DISABLE", "1"], ["TF_IN_AUTOMATION", "1"], ["TF_INPUT", "0"]],
        "kind": "file",
        "resources": "light",
        "shard": "terraform",
    },
    "tflint": {
        "attr": attr.label(default = "@dx_tools//:tflint", allow_single_file = True, cfg = "exec"),
        "env": [["CHECKPOINT_DISABLE", "1"]],
        "kind": "file",
        "resources": "light",
        "shard": "terraform",
    },
}

SHARD_SPECS = {
    "real_terraform_format": {
        "capability": "format",
        "doc": "Additive Terraform format family aspect (terraform fmt).",
        "shard": "terraform",
        "suffix": "-terraform",
    },
    "real_terraform_lint": {
        "capability": "lint",
        "doc": "Additive Terraform lint family aspect (tflint).",
        "shard": "terraform",
        "suffix": "-terraform",
    },
    "real_terraform_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Terraform typecheck family aspect (terraform init and validate, offline).",
        "shard": "terraform",
        "suffix": "-terraform",
    },
}

def _terraform_hook(target, ctx, resolved, capability, what, shared):
    if capability != "typecheck":
        return real_context()

    # Local module sources of deps are staged next to the owned files; the providers
    # are offline mirrors whose roots reach the runner as -plugin-dir values.
    pairs = default_files_hook(["tf", "json", "hcl"])(target, ctx, resolved, capability, what, shared).resolve_pairs
    mirror_roots = {}
    provider_files = []
    if TerraformModuleInfo in target:
        for provider in target[TerraformModuleInfo].transitive_providers.to_list():
            mirror_roots[provider.mirror_root] = True
            provider_files.extend(provider.files.to_list())

    # Relative to the execution root; the runner makes each root absolute. May be empty.
    env = [real_request_tool_env("DX_TERRAFORM_PLUGIN_DIRS", "|".join(sorted(mirror_roots.keys())))]
    return real_context(early_inputs = provider_files, resolve_pairs = pairs, env = env)


HOOK_ORDER = ["terraform"]

HOOKS = {
    "terraform": _terraform_hook,
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

real_terraform_format_aspect = real_make_aspect(_CFG, "real_terraform_format")

real_terraform_lint_aspect = real_make_aspect(_CFG, "real_terraform_lint")

real_terraform_typecheck_aspect = real_make_aspect(_CFG, "real_terraform_typecheck")
