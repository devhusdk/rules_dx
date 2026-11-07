"""TypeScript DocIr shard action shared by the rule and the docs aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

TYPEDOC_JSON_SCHEMA = "2.0"

def typedoc_arguments(tsconfig, package, json, entry_points, options, base_path = "."):
    """Returns the TypeDoc command line for one project."""
    arguments = [
        "--tsconfig",
        tsconfig,
        "--name",
        package,
        "--basePath",
        base_path,
        "--json",
        json,
        "--excludePrivate",
        "--excludeProtected",
        "--excludeInternal",
        "--excludeExternals",
        "--treatWarningsAsErrors",
        "--treatValidationWarningsAsErrors",
    ]
    for entry in entry_points:
        arguments.extend(["--entryPoints", entry])
    return arguments + options

def typescript_shard_action(ctx, typedoc_json, package, source_prefix, config, stem, extract_tool):
    """Converts one project's TypeDoc JSON into a DocIr shard."""
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    args = ctx.actions.args()
    args.add("--typedoc", typedoc_json)
    args.add("--out", shard)
    args.add("--package", package)
    args.add("--source-prefix", source_prefix)
    args.add("--config", config)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [args],
        inputs = [typedoc_json],
        outputs = [shard],
        mnemonic = "DxDocsTypescriptExtract",
        progress_message = "Extracting TypeScript API docs for %{label}",
    )
    return shard
