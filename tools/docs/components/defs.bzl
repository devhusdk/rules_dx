"""Component API reference shards built from the pinned component metadata tools."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("//modules:versions.bzl", "TYPESCRIPT_VERSION", "VUE_COMPONENT_META_VERSION")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

_VUE_COMPONENT_META = Label("//quality/tools/javascript/bin:vue_component_meta")
_META_SCHEMA = "dx-component-meta/1"

def _component_doc_shard_impl(ctx):
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    args = ctx.actions.args()
    args.add("--meta", ctx.file.meta)
    args.add("--out", shard)
    args.add("--language", ctx.attr.language)
    args.add("--package", ctx.attr.package)
    args.add("--source-prefix", ctx.attr.source_prefix)
    args.add("--config", ctx.attr.config)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [args],
        inputs = [ctx.file.meta],
        outputs = [shard],
        mnemonic = "DxDocsComponentExtract",
        progress_message = "Extracting component API docs for %{label}",
    )
    return [
        DefaultInfo(files = depset([shard])),
        DocsShardInfo(language = ctx.attr.language, package = ctx.attr.package, shard = shard),
    ]

component_doc_shard = rule(
    doc = "Converts one project's dx-component-meta/1 JSON into a DocIr shard.",
    implementation = _component_doc_shard_impl,
    attrs = {
        "config": attr.string(doc = "Configuration identity recorded on every symbol."),
        "language": attr.string(mandatory = True, doc = "Language segment of the symbol ids; must equal the metadata framework."),
        "meta": attr.label(allow_single_file = True, mandatory = True, doc = "The dx-component-meta/1 JSON."),
        "package": attr.string(mandatory = True, doc = "Package segment of the symbol ids."),
        "source_prefix": attr.string(doc = "Repo-relative directory of the project."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/components/extract:component_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def vue_doc_shard(name, srcs, tsconfig, entry_points, package, node_modules, package_json = None, deps = [], **kwargs):
    """Declares a component metadata run over one Vue project and its DocIr shard.

    The metadata comes from the pinned `vue-component-meta`, run through an isolated
    launcher: `vueCompilerOptions.plugins` (also inherited ones) is rejected, and no
    Vite config or other consumer JavaScript runs. Only the consumer's `vue` package
    is read from `node_modules`, for its type declarations. Entry points are `.vue`
    files whose default export is the component. Symbol ids are
    `vue:<package>:<Component>` plus `.prop_`, `.event_`, `.slot_` and `.exposed_`
    members.
    """
    where = "vue_doc_shard " + native.package_name() + ":" + name
    if len(entry_points) == 0:
        fail(where + ": need at least one entry point")
    if package == "":
        fail(where + ": package is required")
    meta = name + ".component_meta.json"
    arguments = ["--tsconfig", tsconfig, "--out", meta]
    for entry in entry_points:
        arguments.extend(["--entry", entry])
    js_run_binary(
        name = name + "_meta",
        srcs = srcs + [tsconfig] + ([package_json] if package_json else []) + deps + [node_modules + "/vue"],
        args = arguments,
        chdir = native.package_name(),
        mnemonic = "DxVueComponentMeta",
        outs = [meta],
        tags = kwargs.get("tags", []),
        tool = _VUE_COMPONENT_META,
        visibility = kwargs.get("visibility"),
    )
    component_doc_shard(
        name = name,
        meta = ":" + meta,
        language = "vue",
        package = package,
        source_prefix = native.package_name(),
        config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";meta-json=" + _META_SCHEMA + ";vue-component-meta=" + VUE_COMPONENT_META_VERSION + ";typescript=" + TYPESCRIPT_VERSION,
        **kwargs
    )
