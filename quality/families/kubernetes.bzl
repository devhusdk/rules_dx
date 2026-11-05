"""Kubernetes real quality aspects."""

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
}

SHARD_SPECS = {
    "real_kubernetes_lint": {
        "capability": "lint",
        "doc": "Additive Kubernetes lint family aspect (kubeconform, helm).",
        "shard": "kubernetes",
        "suffix": "-kubernetes",
    },
}

def _helm_context(target, ctx, what):
    """Returns the chart context file for one helm_chart target."""
    if HelmChartInfo not in target:
        fail(what + ": helm needs a helm_chart target")
    chart = target[HelmChartInfo]
    context = ctx.actions.declare_file(target.label.name + "-helm-context.json")
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
        options = {"path_tools": [("kubeconform", ctx.file._kubeconform)]},
    )

HOOK_ORDER = ["helm"]

HOOKS = {"helm": _helm_hook}

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
