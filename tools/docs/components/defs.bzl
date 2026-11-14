"""Component API reference shards built from the pinned component metadata tools."""

load("@aspect_rules_js//js:defs.bzl", "js_run_binary")
load("//core/modules:versions.bzl", "ASTRO_COMPILER_VERSION", "MDX_VERSION", "SVELTE2TSX_VERSION", "SVELTE_VERSION", "TYPESCRIPT_VERSION", "VUE_COMPONENT_META_VERSION")
load("//tools/docs/components:shard.bzl", "component_shard_action")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

_VUE_COMPONENT_META = Label("//javascript/private/quality/bin:vue_component_meta")
_MDX_COMPONENT_META = Label("//javascript/private/quality/bin:mdx_component_meta")
_SVELTE_COMPONENT_META = Label("//javascript/private/quality/bin:svelte_component_meta")
_ASTRO_COMPONENT_META = Label("//javascript/private/quality/bin:astro_component_meta")
_META_SCHEMA = "dx-component-meta/1"

def _component_doc_shard_impl(ctx):
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    info = component_shard_action(ctx, ctx.executable._extract, ctx.file.meta, shard, ctx.attr.language, ctx.attr.package, ctx.attr.source_prefix, ctx.attr.config)
    return [DefaultInfo(files = depset([shard])), info]

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
    """Declares a component metadata run over one Vue project and its DocIr shard."""
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
        resource_set = "mem_2g",
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

def svelte_doc_shard(name, srcs, tsconfig, entry_points, package, node_modules, package_json = None, deps = [], **kwargs):
    """Declares a component metadata run over one Svelte project and its DocIr shard."""
    where = "svelte_doc_shard " + native.package_name() + ":" + name
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
        srcs = srcs + [tsconfig] + ([package_json] if package_json else []) + deps + [node_modules + "/svelte"],
        args = arguments,
        chdir = native.package_name(),
        mnemonic = "DxSvelteComponentMeta",
        resource_set = "mem_2g",
        outs = [meta],
        tags = kwargs.get("tags", []),
        tool = _SVELTE_COMPONENT_META,
        visibility = kwargs.get("visibility"),
    )
    component_doc_shard(
        name = name,
        meta = ":" + meta,
        language = "svelte",
        package = package,
        source_prefix = native.package_name(),
        config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";meta-json=" + _META_SCHEMA + ";svelte2tsx=" + SVELTE2TSX_VERSION + ";svelte=" + SVELTE_VERSION + ";typescript=" + TYPESCRIPT_VERSION,
        **kwargs
    )

def astro_doc_shard(name, srcs, tsconfig, entry_points, package, node_modules, package_json = None, deps = [], **kwargs):
    """Declares a component metadata run over one Astro project and its DocIr shard."""
    where = "astro_doc_shard " + native.package_name() + ":" + name
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
        srcs = srcs + [tsconfig] + ([package_json] if package_json else []) + deps + [node_modules + "/astro"],
        args = arguments,
        chdir = native.package_name(),
        mnemonic = "DxAstroComponentMeta",
        resource_set = "mem_2g",
        outs = [meta],
        tags = kwargs.get("tags", []),
        tool = _ASTRO_COMPONENT_META,
        visibility = kwargs.get("visibility"),
    )
    component_doc_shard(
        name = name,
        meta = ":" + meta,
        language = "astro",
        package = package,
        source_prefix = native.package_name(),
        config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";meta-json=" + _META_SCHEMA + ";astro-compiler=" + ASTRO_COMPILER_VERSION + ";typescript=" + TYPESCRIPT_VERSION,
        **kwargs
    )

def mdx_doc_shard(name, srcs, tsconfig, entry_points, package, deps = [], **kwargs):
    """Declares a component metadata run over one MDX project and its DocIr shard."""
    where = "mdx_doc_shard " + native.package_name() + ":" + name
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
        srcs = srcs + [tsconfig] + deps,
        args = arguments,
        chdir = native.package_name(),
        mnemonic = "DxMdxComponentMeta",
        resource_set = "mem_2g",
        outs = [meta],
        tags = kwargs.get("tags", []),
        tool = _MDX_COMPONENT_META,
        visibility = kwargs.get("visibility"),
    )
    component_doc_shard(
        name = name,
        meta = ":" + meta,
        language = "mdx",
        package = package,
        source_prefix = native.package_name(),
        config = "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";meta-json=" + _META_SCHEMA + ";mdx=" + MDX_VERSION + ";typescript=" + TYPESCRIPT_VERSION,
        **kwargs
    )
