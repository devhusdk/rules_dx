"""Component API reference shard action, free of any JavaScript rule set."""

load("//core/modules:versions.bzl", "ASTRO_COMPILER_VERSION", "MDX_VERSION", "SVELTE2TSX_VERSION", "SVELTE_VERSION", "TYPESCRIPT_VERSION", "VUE_COMPONENT_META_VERSION")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

META_SCHEMA = "dx-component-meta/1"

COMPONENT_LANGUAGES = {
    "astro": struct(extension = "astro", framework = "astro", mnemonic = "DxAstroComponentMeta", tools = "astro-compiler=" + ASTRO_COMPILER_VERSION),
    "mdx": struct(extension = "mdx", framework = None, mnemonic = "DxMdxComponentMeta", tools = "mdx=" + MDX_VERSION),
    "svelte": struct(extension = "svelte", framework = "svelte", mnemonic = "DxSvelteComponentMeta", tools = "svelte2tsx=" + SVELTE2TSX_VERSION + ";svelte=" + SVELTE_VERSION),
    "vue": struct(extension = "vue", framework = "vue", mnemonic = "DxVueComponentMeta", tools = "vue-component-meta=" + VUE_COMPONENT_META_VERSION),
}

def component_config(language, tsconfig, entry_points):
    """Returns the configuration identity recorded on every symbol of one project."""
    return "tsconfig=" + tsconfig + ";entry=" + ",".join(entry_points) + ";meta-json=" + META_SCHEMA + ";" + COMPONENT_LANGUAGES[language].tools + ";typescript=" + TYPESCRIPT_VERSION

def component_shard_action(ctx, extract, meta, shard, language, package, source_prefix, config):
    """Converts one dx-component-meta/1 JSON into a DocIr shard.

    Args:
        ctx: The rule or aspect context.
        extract: The component extractor executable.
        meta: The dx-component-meta/1 JSON file.
        shard: The declared DocIr shard output.
        language: Language segment of the symbol ids.
        package: Package segment of the symbol ids.
        source_prefix: Repo-relative directory of the project.
        config: Configuration identity recorded on every symbol.

    Returns:
        The DocsShardInfo of the shard.
    """
    args = ctx.actions.args()
    args.add("--meta", meta)
    args.add("--out", shard)
    args.add("--language", language)
    args.add("--package", package)
    args.add("--source-prefix", source_prefix)
    args.add("--config", config)
    ctx.actions.run(
        executable = extract,
        arguments = [args],
        inputs = [meta],
        outputs = [shard],
        mnemonic = "DxDocsComponentExtract",
        progress_message = "Extracting component API docs for %{label}",
    )
    return DocsShardInfo(language = language, package = package, shard = shard)
