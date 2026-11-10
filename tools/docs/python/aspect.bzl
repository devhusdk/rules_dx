"""Python documentation shard aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/python:shard.bzl", "python_package_name_ok", "python_shard_action")
load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")

def _python_docs_shard(target, ctx):
    if _PyInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    if not python_package_name_ok(ctx.label.name):
        return None
    action = python_shard_action(ctx, target[_PyInfo], ctx.label.name, "google", "_dx_docs/" + ctx.label.name, ctx.executable._py_driver, ctx.executable._py_extract)
    return action.shard if action else None

def _real_python_docs_aspect_impl(target, ctx):
    shard = _python_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_python_docs_aspect = aspect(
    implementation = _real_python_docs_aspect_impl,
    doc = "Puts the Python docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_py_driver": attr.label(
            cfg = "exec",
            default = Label("//core/quality/tools/python:griffe_dump"),
            executable = True,
        ),
        "_py_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/python/extract:python_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
