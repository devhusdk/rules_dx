"""Target-scoped capability aspects over synthetic adapters."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load(
    "//quality:adapters.bzl",
    "SYNTHETIC_ADAPTERS",
    "SYNTHETIC_CLASS_TO_FAMILY",
)
load("//quality:execution_requirements.bzl", "dx_action_requirements", "dx_execution_requirements")
load("//quality:pipeline.bzl", "aspect_capability_blocked", "aspect_direct_maps", "aspect_family_selections", "ordered_pipeline_paths", "pipeline_inputs_for_paths", "resolve_pipeline", "stage_flag")
load("//quality:policy.bzl", "QualityPolicyInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

def _quality_pipeline_action(target, ctx, capability):
    if QualitySourcesInfo not in target:
        return []
    if aspect_capability_blocked(ctx.rule.attr, capability):
        return []
    info = target[QualitySourcesInfo]
    policy = ctx.attr._policy[QualityPolicyInfo]

    (target_classes, _, direct_paths, path_to_file) = aspect_direct_maps(info.direct_sources, "quality_aspect (" + str(target.label) + ")")
    selections = aspect_family_selections(policy, capability)
    resolved = resolve_pipeline(
        target_classes,
        direct_paths,
        capability,
        selections,
        SYNTHETIC_CLASS_TO_FAMILY,
        SYNTHETIC_ADAPTERS,
    )
    if len(resolved) == 0:
        return []

    out = ctx.actions.declare_file(target.label.name + "-" + capability + ".pb")

    ordered_paths = ordered_pipeline_paths(resolved)
    inputs = pipeline_inputs_for_paths(ordered_paths, path_to_file)

    args = ctx.actions.args()
    args.add("--producer", str(target.label))
    args.add("--capability", capability)
    args.add("--output", out.path)
    for stage in resolved:
        args.add("--stage", stage_flag(stage))
    for ws_path in ordered_paths:
        f = path_to_file.get(ws_path)
        if f != None:
            args.add("--source", ws_path + "=" + f.path)

    ctx.actions.run(
        executable = ctx.executable._runner,
        inputs = depset(inputs),
        outputs = [out],
        arguments = [args],
        execution_requirements = dx_action_requirements([stage["tool"] for stage in resolved], capability),
        mnemonic = "DxQuality" + capability.capitalize(),
        progress_message = "Dx quality " + capability + " %{label}",
    )

    if not ctx.attr._validate[BuildSettingInfo].value:
        return [OutputGroupInfo(dx_results = depset([out]))]
    marker = ctx.actions.declare_file(target.label.name + "-" + capability + ".validated")
    eval_args = ctx.actions.args()
    eval_args.add("--result", out.path)
    eval_args.add("--fail_on", ctx.attr._fail_on[BuildSettingInfo].value)
    eval_args.add("--output", marker.path)
    ctx.actions.run(
        executable = ctx.executable._evaluator,
        inputs = depset([out]),
        outputs = [marker],
        arguments = [eval_args],
        execution_requirements = dx_execution_requirements(),
        mnemonic = "DxQualityEval",
        progress_message = "Dx quality evaluate " + capability + " %{label}",
    )
    return [OutputGroupInfo(dx_results = depset([out, marker]))]

def _make_synthetic_impl(capability):
    """Makes one capability impl over the shared pipeline action."""

    def _impl(target, ctx):
        return _quality_pipeline_action(target, ctx, capability)

    return _impl

_SYNTHETIC_CAPABILITIES = ["lint", "format", "typecheck", "audit"]

_COMMON_ATTRS = {
    "_evaluator": attr.label(
        default = "//quality/evaluator:quality_evaluator",
        executable = True,
        cfg = "exec",
        allow_files = True,
    ),
    "_fail_on": attr.label(
        default = "//config:fail_on",
        providers = [BuildSettingInfo],
    ),
    "_policy": attr.label(
        default = "//quality:fixture_policy",
        providers = [QualityPolicyInfo],
    ),
    "_runner": attr.label(
        default = "//quality/runner:quality_runner",
        executable = True,
        cfg = "exec",
        allow_files = True,
    ),
    "_validate": attr.label(
        default = "//config:validate",
        providers = [BuildSettingInfo],
    ),
}

_SYNTHETIC_ASPECTS = {
    capability: aspect(
        implementation = _make_synthetic_impl(capability),
        attr_aspects = [],
        attrs = _COMMON_ATTRS,
    )
    for capability in _SYNTHETIC_CAPABILITIES
}

lint_aspect = _SYNTHETIC_ASPECTS["lint"]

format_aspect = _SYNTHETIC_ASPECTS["format"]

typecheck_aspect = _SYNTHETIC_ASPECTS["typecheck"]

audit_aspect = _SYNTHETIC_ASPECTS["audit"]
