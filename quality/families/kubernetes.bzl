"""Kubernetes real quality aspects."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("//helm/rules:providers.bzl", "HelmChartInfo")
load("//quality:family_hooks.bzl", "stage")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")

TOOLS = {
    "helm": {
        "attr": attr.label(default = "//config:tool_helm", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "kubernetes",
    },
    "kubeconform": {
        "attr": attr.label(default = "//config:tool_kubeconform", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "kubernetes",
    },
    "trivy_config": {
        "attr": attr.label(default = "//config:tool_trivy", allow_single_file = True, cfg = "exec"),
        "kind": "file",
        "resources": "light",
        "shard": "kubernetes",
    },
}

SHARD_SPECS = {
    "real_kubernetes_lint": {
        "capability": "lint",
        "doc": "Additive Kubernetes lint family aspect (kubeconform, helm).",
        "extra_attrs": {
            "_config_profile": attr.label(default = "//config:config_profile"),
        },
        "shard": "kubernetes",
        "suffix": "-kubernetes",
    },
}

def _helm_context(target, ctx, what, tool = "helm"):
    """Returns the chart context file for one helm_chart target."""
    if HelmChartInfo not in target:
        fail(what + ": helm needs a helm_chart target")
    chart = target[HelmChartInfo]
    context = ctx.actions.declare_file(target.label.name + "-" + tool + "-context.json")
    ctx.actions.write(
        output = context,
        content = json.encode({
            "chart_root": chart.chart_root,
            "namespace": chart.namespace,
            "release": chart.release_name,
            "schema_version": 1,
            "values": [f.short_path for f in chart.values],
        }),
    )
    return context

def _tier_args(ctx):
    """Returns ["strict"] on the strict tier, which makes kubeconform pass -strict."""
    return ["strict"] if ctx.attr._config_profile[BuildSettingInfo].value == "strict" else []

def _kubeconform_hook(_target, ctx, _resolved, _capability, _what, _shared):
    return real_context(options = {"args": _tier_args(ctx)})

def _helm_hook(target, ctx, _resolved, _capability, what, _shared):
    context = _helm_context(target, ctx, what)
    tool_files = []
    inputs = []
    stage(tool_files, inputs, "dx-helm/context.json", context)
    inputs.append(ctx.file._kubeconform)
    pairs = []
    if HelmChartInfo in target:
        for f in target[HelmChartInfo].dependencies:
            ws_path = f.short_path
            if ".." in ws_path.split("/") or ws_path.startswith("../"):
                continue
            pairs.append((ws_path, f, False))
    return real_context(
        tool_files = tool_files,
        inputs = inputs,
        resolve_pairs = pairs,
        options = {"args": _tier_args(ctx), "path_tools": [("kubeconform", ctx.file._kubeconform)]},
    )

def _trivy_hook(target, ctx, _resolved, _capability, what, _shared):
    if HelmChartInfo not in target:
        return real_context()
    context = _helm_context(target, ctx, what, "trivy")
    tool_files = []
    inputs = []
    stage(tool_files, inputs, "dx-helm/context.json", context)
    pairs = []
    for f in target[HelmChartInfo].dependencies:
        ws_path = f.short_path
        if ".." in ws_path.split("/") or ws_path.startswith("../"):
            continue
        pairs.append((ws_path, f, False))
    return real_context(tool_files = tool_files, inputs = inputs, resolve_pairs = pairs)

HOOK_ORDER = ["helm", "kubeconform", "trivy_config"]

HOOKS = {"helm": _helm_hook, "kubeconform": _kubeconform_hook, "trivy_config": _trivy_hook}

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

real_kubernetes_lint_aspect = real_make_aspect(_CFG, "real_kubernetes_lint")
