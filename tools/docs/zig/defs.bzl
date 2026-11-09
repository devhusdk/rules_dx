"""Zig API reference shards from the pinned compiler's own parser (`std.zig.Ast`)."""

load("@rules_zig//zig/private/providers:zig_module_info.bzl", "ZigModuleInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative", "srcs_manifest_line")

_ZigDirectSrcsInfo = provider(
    doc = "The .zig files one Zig library lists in its own srcs, main file first.",
    fields = {"srcs": "List[File]: the library's direct Zig srcs."},
)

def _direct_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_ZigDirectSrcsInfo(srcs = [f for f in srcs if f.extension == "zig"])]

_direct_srcs_aspect = aspect(
    doc = "Reads the direct Zig srcs of a zig_library, through its public forwarder.",
    implementation = _direct_srcs_aspect_impl,
)

def zig_shard_action(ctx, own, package, config, stem, dump_tool, extract_tool):
    """Dumps a Zig library's own sources with `std.zig.Ast` and extracts its DocIr shard.

    Args:
      ctx: The rule or aspect context.
      own: The .zig files the library lists in its own srcs; the main file comes first.
      package: Package segment of the shard's symbol ids.
      config: Extra configuration identity recorded on every symbol.
      stem: Output name prefix, relative to the target's package.
      dump_tool: The `zig_doc_dump` executable.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard`, the parser `dump` and the `srcs_manifest`.
    """
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join([srcs_manifest_line(f.path, repo_relative(f.short_path), not f.is_source) for f in own]))

    dump = ctx.actions.declare_file(stem + ".dump.json")
    args = ctx.actions.args()
    args.add("--out", dump)
    args.add_all(own)
    ctx.actions.run(
        executable = dump_tool,
        arguments = [args],
        inputs = own,
        outputs = [dump],
        mnemonic = "DxDocsZigDump",
        toolchain = None,
        progress_message = "Reading Zig API docs for %{label}",
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--dump", dump)
    extract.add("--sources", manifest)
    extract.add("--package", package)
    extract.add("--config", config)
    extract.add("--out", shard)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [dump, manifest],
        outputs = [shard],
        mnemonic = "DxDocsZigExtract",
        toolchain = None,
        progress_message = "Extracting Zig API docs for %{label}",
    )
    return struct(shard = shard, dump = dump, srcs_manifest = manifest)

def _zig_doc_shard_impl(ctx):
    lib = ctx.attr.lib
    own = lib[_ZigDirectSrcsInfo].srcs
    if not own:
        fail("zig_doc_shard " + str(ctx.label) + ": lib lists no .zig srcs")
    package = ctx.attr.package or lib.label.name
    action = zig_shard_action(ctx, own, package, ctx.attr.config, ctx.label.name, ctx.executable._dump, ctx.executable._extract)
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(
            dump = depset([action.dump]),
            srcs_manifest = depset([action.srcs_manifest]),
        ),
        DocsShardInfo(language = "zig", package = package, shard = action.shard),
    ]

zig_doc_shard = rule(
    doc = "Converts one Zig library's public API into a DocIr shard with the pinned Zig's std.zig.Ast.",
    implementation = _zig_doc_shard_impl,
    attrs = {
        "config": attr.string(doc = "Extra configuration identity recorded on every symbol."),
        "lib": attr.label(
            mandatory = True,
            providers = [ZigModuleInfo],
            aspects = [_direct_srcs_aspect],
            doc = "The zig_library to document.",
        ),
        "package": attr.string(doc = "Package segment of the symbol ids. Defaults to the lib name."),
        "_dump": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/zig/dump:zig_doc_dump"),
            executable = True,
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/zig/extract:zig_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)
