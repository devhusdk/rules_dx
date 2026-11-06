"""Analysis subject exposing an aggregate workspace policy."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load("//quality:native_config.bzl", "DxNativeConfigInfo")
load(":policy.bzl", "CAPABILITIES", "QualityPolicyInfo")

def _policy_subject_impl(ctx):
    fields = {}
    for family_id in sorted(ctx.attr.policy[QualityPolicyInfo].families.keys()):
        section = ctx.attr.policy[QualityPolicyInfo].families[family_id]
        for capability in CAPABILITIES:
            fields["family." + family_id + "." + capability] = ",".join(
                getattr(section, capability),
            )
            if section.profile:
                fields["profile." + family_id] = section.profile
            fields["disabled." + family_id + "." + capability] = str(
                capability in section.disabled,
            )
    configs = ctx.attr.policy[QualityPolicyInfo].configs
    for tool_id in sorted(configs.keys()):
        fields["config." + tool_id] = configs[tool_id][DxNativeConfigInfo].config.short_path
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = fields),
    ]

policy_subject = rule(
    implementation = _policy_subject_impl,
    doc = "Analysis subject reporting a workspace quality policy.",
    attrs = {
        "policy": attr.label(
            doc = "Quality policy target whose families are reported as subject fields.",
            providers = [QualityPolicyInfo],
        ),
    },
)
