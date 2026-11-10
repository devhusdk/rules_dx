"""Concrete configuration-subjects use case."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("//core:defs.bzl", "DxConfigInfo", "DxSubjectInfo")

def _config_flip_transition_impl(_settings, _attr):
    """Flips the futures config_value setting for deps."""
    return {"//core/tests/fixtures/starlark_futures:config_value": "flipped"}

config_flip_transition = transition(
    implementation = _config_flip_transition_impl,
    inputs = [],
    outputs = ["//core/tests/fixtures/starlark_futures:config_value"],
)

def _config_leaf_impl(ctx):
    """Exposes configurable attribute plus fragment plus setting values."""
    config_value = ctx.attr._config_value[BuildSettingInfo].value
    has_platform = hasattr(ctx.fragments, "platform")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "config=" + config_value + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = {
            "config_value": config_value,
            "note": ctx.attr.note,
        }),
        DxConfigInfo(fields = {
            "config_value": config_value,
            "fragment_platform": "present" if has_platform else "absent",
            "note": ctx.attr.note,
            "transition": "leaf",
        }),
    ]

config_leaf = rule(
    implementation = _config_leaf_impl,
    fragments = ["platform"],
    attrs = {
        "note": attr.string(default = ""),
        "_config_value": attr.label(
            default = "//core/tests/fixtures/starlark_futures:config_value",
        ),
    },
)

def _config_group_impl(ctx):
    """Exposes its own config plus the transitioned dep value."""
    config_value = ctx.attr._config_value[BuildSettingInfo].value
    dep_value = ""
    dep_note = ""
    if len(ctx.attr.deps) > 0:
        dep_value = ctx.attr.deps[0][DxConfigInfo].fields.get("config_value", "")
        dep_note = ctx.attr.deps[0][DxConfigInfo].fields.get("note", "")
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "config=" + config_value + " dep=" + dep_value + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = {
            "config_value": config_value,
            "dep_config": dep_value,
            "dep_note": dep_note,
            "note": ctx.attr.note,
        }),
        DxConfigInfo(fields = {
            "config_value": config_value,
            "dep_config": dep_value,
            "dep_note": dep_note,
            "fragment_platform": "present" if hasattr(ctx.fragments, "platform") else "absent",
            "note": ctx.attr.note,
            "transition": "group",
        }),
    ]

config_group = rule(
    implementation = _config_group_impl,
    fragments = ["platform"],
    attrs = {
        "deps": attr.label_list(cfg = config_flip_transition),
        "note": attr.string(default = ""),
        "_config_value": attr.label(
            default = "//core/tests/fixtures/starlark_futures:config_value",
        ),
    },
)
