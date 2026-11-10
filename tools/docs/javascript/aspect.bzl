"""JavaScript documentation shard aspect."""

load("//core:resources.bzl", "dx_heavy_resources")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")
load("//tools/docs/typescript:shard.bzl", "TYPEDOC_JSON_SCHEMA", "typedoc_arguments", "typescript_shard_action")
load("@aspect_rules_js//js:libs.bzl", "js_binary_lib")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")

_TS_TEST_MARKERS = [".test.", ".spec.", "_test."]

_JS_ENTRY_EXTENSIONS = ["js", "mjs", "cjs", "jsx"]

def _javascript_docs_shard(target, ctx):
    if _JsInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    if not srcs or not all([f.extension in _JS_ENTRY_EXTENSIONS for f in srcs]):
        return None
    js = target[_JsInfo]
    package = ctx.label.package
    tsconfigs = [f for f in js.transitive_sources.to_list() if f.basename == "tsconfig.json" and f.dirname.endswith("/" + package if package else "")]
    if len(tsconfigs) != 1:
        return None
    entry_points = sorted([repo_relative(f.short_path) for f in srcs if not any([marker in f.basename for marker in _TS_TEST_MARKERS])])
    if not entry_points:
        return None
    tsconfig = repo_relative(tsconfigs[0].short_path)
    stem = "_dx_docs/" + ctx.label.name
    typedoc_json = ctx.actions.declare_file(stem + ".typedoc.json")
    arguments = ctx.actions.args()
    arguments.add_all(typedoc_arguments(tsconfig, ctx.label.name, typedoc_json.path[len(ctx.bin_dir.path) + 1:], entry_points, [], package or "."))
    js_binary_lib.run_binary_action(
        ctx,
        mnemonic = "DxTypedoc",
        progress_message = "TypeDoc JSON of %{label}",
        executable = ctx.attr._typedoc[DefaultInfo].files_to_run,
        inputs = depset(transitive = [js.transitive_sources, js.npm_sources]),
        outputs = [typedoc_json],
        arguments = [arguments],
        resource_set = dx_heavy_resources,
    )
    config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";typedoc-json=" + TYPEDOC_JSON_SCHEMA + ";options="
    return typescript_shard_action(ctx, typedoc_json, ctx.label.name, package, config, stem, ctx.executable._ts_extract)

def _real_javascript_docs_aspect_impl(target, ctx):
    shard = _javascript_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_javascript_docs_aspect = aspect(
    implementation = _real_javascript_docs_aspect_impl,
    doc = "Puts the JavaScript docs shard of a target in dx_results and dx_docs_shards.",
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
