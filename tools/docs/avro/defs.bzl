"""Avro schema reference shards read from the `doc` attribute of the schemas."""

load("//avro/rules:providers.bzl", "AvroInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")

def avro_shard_action(ctx, srcs, imports, avro_tools, package, stem, extract_tool):
    """Runs avro-tools on each .avdl and the extractor over every schema of one avro_library.

    Args:
      ctx: The rule or aspect context.
      srcs: The .avsc and .avdl files the library lists in its own srcs.
      imports: Every schema the library reaches, so .avdl imports resolve.
      avro_tools: The FilesToRunProvider of the pinned avro-tools (//config:tool_avro).
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`.
    """
    if not srcs:
        fail("avro_doc_shard " + str(ctx.label) + ": avro_library lists no schema srcs")
    extract = ctx.actions.args()
    extract.add("--package", package)
    inputs = []
    for src in sorted(srcs, key = lambda f: f.path):
        source = repo_relative(src.short_path)
        if not src.is_source:
            extract.add("--generated", source)
        if src.extension == "avdl":
            # idl2schemata writes one .avsc per named type; messages stay in the .avdl.
            tree = ctx.actions.declare_directory("%s.%s.schemata" % (stem, src.basename))
            args = ctx.actions.args()
            args.add_all(["idl2schemata", src, tree.path])
            ctx.actions.run(
                executable = avro_tools,
                arguments = [args],
                inputs = depset([src], transitive = [imports]),
                outputs = [tree],
                mnemonic = "DxDocsAvroIdl",
                toolchain = None,
                progress_message = "Reading Avro IDL docs of " + src.short_path,
            )
            inputs.append(tree)
            extract.add("--schema-dir", "%s=%s" % (tree.path, source))
        else:
            inputs.append(src)
            extract.add("--schema", "%s=%s" % (src.path, source))
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = inputs,
        outputs = [shard],
        mnemonic = "DxDocsAvroExtract",
        toolchain = None,
        progress_message = "Extracting Avro docs for %{label}",
    )
    return struct(shard = shard)

def _avro_doc_shard_impl(ctx):
    info = ctx.attr.avro[AvroInfo]
    package = ctx.attr.package or ctx.attr.avro.label.name
    action = avro_shard_action(
        ctx,
        info.direct_srcs,
        info.transitive_srcs,
        ctx.attr._avro_tools[DefaultInfo].files_to_run,
        package,
        ctx.label.name,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        DocsShardInfo(language = "avro", package = package, shard = action.shard),
    ]

docs_avro_shard = rule(
    doc = "Converts one avro_library's records, enums, fixed types and protocol messages into a DocIr shard.",
    implementation = _avro_doc_shard_impl,
    attrs = {
        "avro": attr.label(
            mandatory = True,
            providers = [AvroInfo],
            doc = "The documented avro_library; its own schemas are documented.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of avro."),
        "_avro_tools": attr.label(default = Label("//config:tool_avro"), executable = True, cfg = "exec"),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/avro/extract:avro_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def avro_doc_shard(name, avro, package = "", **kwargs):
    """Declares the DocIr shard of one avro_library.

    Args:
      name: Name of the shard target.
      avro: The documented avro_library.
      package: Package segment of every symbol id; defaults to the name of avro.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_avro_shard(
        name = name,
        avro = avro,
        package = package,
        **kwargs
    )
