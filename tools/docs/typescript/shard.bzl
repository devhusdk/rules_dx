"""TypeScript DocIr shard action shared by the rule and the docs aspect."""

load("//tools/docs/ir:shard.bzl", "DocsShardInfo")

TYPEDOC_JSON_SCHEMA = "2.0"

def typedoc_arguments(tsconfig, package, json, entry_points, options, base_path = "."):
    """Returns the TypeDoc command line for one project.

    Args:
        tsconfig: Path of the tsconfig, relative to the TypeDoc working directory.
        package: Project name passed to TypeDoc.
        json: Path of the TypeDoc JSON output.
        entry_points: Entry point paths, relative to the working directory.
        options: Extra TypeDoc flags.
        base_path: The project directory, relative to the working directory.

    Returns:
        The argument list.
    """
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
    """Converts one project's TypeDoc JSON into a DocIr shard.

    Args:
        ctx: The rule or aspect context.
        typedoc_json: The TypeDoc JSON file.
        package: Package segment of the symbol ids.
        source_prefix: Repo-relative directory of the TypeScript project.
        config: Configuration identity recorded on every symbol.
        stem: Output name prefix.
        extract_tool: The DocIr extractor executable.

    Returns:
        The DocIr shard file.
    """
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
