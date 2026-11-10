"""TypeScript documentation shard aspect."""

load("//core:resources.bzl", "dx_heavy_resources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")
load("//tools/docs/typescript:shard.bzl", "TYPEDOC_JSON_SCHEMA", "typedoc_arguments", "typescript_shard_action")
load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")

_TS_ENTRY_EXTENSIONS = ["ts", "tsx", "mts", "cts"]

_TS_TEST_MARKERS = [".test.", ".spec.", "_test."]

def _typescript_entry_point(file):
    name = file.basename
    if file.extension not in _TS_ENTRY_EXTENSIONS or ".d." in name:
        return False
    return not any([marker in name for marker in _TS_TEST_MARKERS])

def _typescript_docs_shard(target, ctx):
    attrs = ctx.rule.attr
    if DocsShardInfo in target or not (hasattr(attrs, "tsc") and hasattr(attrs, "_options") and hasattr(attrs, "tsconfig")):
        return None
    if ctx.rule.kind.endswith("_test") or OutputGroupInfo not in target or not hasattr(target[OutputGroupInfo], "_action_inputs"):
        return None
    entry_points = sorted([repo_relative(f.short_path) for f in ctx.rule.files.srcs if _typescript_entry_point(f)])
    if not entry_points:
        return None
    package = ctx.label.name
    tsconfig = repo_relative(ctx.rule.file.tsconfig.short_path)
    stem = "_dx_docs/" + ctx.label.name
    typedoc_json = ctx.actions.declare_file(stem + ".typedoc.json")
    arguments = ctx.actions.args()
    arguments.add_all(typedoc_arguments(tsconfig, package, typedoc_json.path[len(ctx.bin_dir.path) + 1:], entry_points, [], ctx.label.package or "."))
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "DxTypedoc",
        progress_message = "TypeDoc JSON of %{label}",
        executable = ctx.attr._typedoc[DefaultInfo].files_to_run,
        inputs = target[OutputGroupInfo]._action_inputs,
        outputs = [typedoc_json],
        arguments = [arguments],
        resource_set = dx_heavy_resources,
    )
    config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";typedoc-json=" + TYPEDOC_JSON_SCHEMA + ";options="
    return typescript_shard_action(ctx, typedoc_json, package, ctx.label.package, config, stem, ctx.executable._ts_extract)

def _real_typescript_docs_aspect_impl(target, ctx):
    shard = _typescript_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_typescript_docs_aspect = aspect(
    implementation = _real_typescript_docs_aspect_impl,
    doc = "Puts the TypeScript docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_ts_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/typescript/extract:typescript_extract"),
            executable = True,
        ),
        "_typedoc": attr.label(
            cfg = "exec",
            default = Label("//javascript/private/quality/bin:typedoc"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
)
