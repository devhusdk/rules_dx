"""Vue, Svelte, Astro and MDX documentation shard aspect."""

load("//tools/docs/components:shard.bzl", "COMPONENT_LANGUAGES", "component_config", "component_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")

def _component_docs_shard(target, ctx):
    if _JsInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    for language, spec in COMPONENT_LANGUAGES.items():
        if srcs and all([f.extension == spec.extension for f in srcs]):
            return _component_shard(target[_JsInfo], ctx, language, srcs)
    return None

def _component_shard(js, ctx, language, srcs):
    inputs = depset(transitive = [js.transitive_sources, js.npm_sources])
    package = ctx.label.package
    tsconfigs = [f for f in js.transitive_sources.to_list() if f.basename == "tsconfig.json" and f.dirname.endswith("/" + package if package else "")]
    if len(tsconfigs) != 1:
        return None
    stem = "_dx_docs/" + ctx.label.name
    meta = ctx.actions.declare_file(stem + ".component_meta.json")
    entry_points = [f.short_path[len(package) + 1:] if package else f.short_path for f in srcs]
    tsconfig = tsconfigs[0].basename
    args = ctx.actions.args()
    args.add("--bazel-bindir", ctx.bin_dir.path)
    args.add("--tsconfig", tsconfig)
    args.add("--out", stem + ".component_meta.json")
    for entry in entry_points:
        args.add("--entry", entry)
    tool = getattr(ctx.attr, "_" + language + "_meta")
    ctx.actions.run(
        executable = getattr(ctx.executable, "_" + language + "_meta"),
        arguments = [args],
        inputs = inputs,
        tools = [tool[DefaultInfo].files_to_run],
        outputs = [meta],
        mnemonic = COMPONENT_LANGUAGES[language].mnemonic,
        progress_message = "Reading component metadata of %{label}",
        env = {"JS_BINARY__CHDIR": package, "JS_BINARY__PATCH_NODE_FS": "1"},
        resource_set = _mem_2g,
    )
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    component_shard_action(ctx, ctx.executable._component_extract, meta, shard, language, ctx.label.name, package, component_config(language, tsconfig, entry_points))
    return shard

def _mem_2g(_os, _inputs):
    return {"memory": 2048}

def _component_meta_attr(language):
    return attr.label(
        cfg = "exec",
        default = Label("//javascript/private/quality/bin:" + language + "_component_meta"),
        executable = True,
    )

def _real_components_docs_aspect_impl(target, ctx):
    shard = _component_docs_shard(target, ctx)
    if not shard:
        return []
    return [OutputGroupInfo(dx_results = depset([shard]), dx_docs_shards = depset([shard]))]

real_components_docs_aspect = aspect(
    implementation = _real_components_docs_aspect_impl,
    doc = "Puts the Vue, Svelte, Astro and MDX docs shard of a target in dx_results and dx_docs_shards.",
    attrs = {
        "_astro_meta": _component_meta_attr("astro"),
        "_component_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/components/extract:component_extract"),
            executable = True,
        ),
        "_mdx_meta": _component_meta_attr("mdx"),
        "_svelte_meta": _component_meta_attr("svelte"),
        "_vue_meta": _component_meta_attr("vue"),
    },
    fragments = ["cpp"],
)
