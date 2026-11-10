"""Swift API reference shards from the symbol graphs of the toolchain's swift-symbolgraph-extract."""

load("//swift/private:providers.bzl", "SwiftDocsInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

def swift_shard_action(ctx, info, package, config, stem, extract_tool):
    """Extracts the DocIr shard of one Swift library from its symbol graph and its own sources.

    Args:
      ctx: The rule or aspect context.
      info: The library's SwiftDocsInfo; it must hold exactly one symbol graph.
      package: Package segment of the shard's symbol ids.
      config: Extra configuration identity recorded on every symbol.
      stem: Output name prefix, relative to the target's package.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard` and the `srcs_manifest`.
    """
    if len(info.graphs) != 1:
        fail("swift_doc_shard " + str(ctx.label) + ": want exactly one symbol graph, got " + str(len(info.graphs)))
    graph = info.graphs[0]
    own = sorted(info.srcs, key = lambda f: f.path)
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own]))
    shard = ctx.actions.declare_file(stem + ".ir.pb")
    args = ctx.actions.args()
    args.add("--graphs", graph.symbol_graph_dir.path)
    args.add("--sources", manifest)
    args.add("--module", graph.module_name)
    args.add("--package", package)
    args.add("--config", config)
    args.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [args],
        inputs = depset(own + [graph.symbol_graph_dir, manifest]),
        outputs = [shard],
        mnemonic = "DxDocsSwiftExtract",
        toolchain = None,
        progress_message = "Extracting Swift API docs for %{label}",
    )
    return struct(shard = shard, srcs_manifest = manifest)

def _swift_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    if not lib[SwiftDocsInfo].srcs:
        fail("swift_doc_shard " + str(ctx.label) + ": lib has no Swift srcs")
    package = ctx.attr.package or lib.label.name
    action = swift_shard_action(ctx, lib[SwiftDocsInfo], package, ctx.attr.config, ctx.label.name, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(srcs_manifest = depset([action.srcs_manifest])),
        DocsShardInfo(language = "swift", package = package, shard = action.shard),
    ]

swift_doc_shard = rule(
    doc = "Converts one Swift library's public API into a DocIr shard with the symbol graph of the toolchain's swift-symbolgraph-extract.",
    implementation = _swift_doc_shard_impl,
    attrs = {
        "config": attr.string(doc = "Extra configuration identity recorded on every symbol."),
        "lib": attr.label(
            mandatory = True,
            providers = [SwiftDocsInfo],
            doc = "The swift_library to document.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids. Defaults to the lib name."),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/swift/extract:swift_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)
