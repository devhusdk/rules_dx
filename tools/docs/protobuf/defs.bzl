"""Protobuf schema reference shards read from protoc's descriptor set and its source info."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")
load("//libs/exec_os:defs.bzl", "ExecOsInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", "repo_relative")

PROTO_TOOLCHAIN = "@protobuf//bazel/private:proto_toolchain_type"

def _root_relative(src, root):
    path = src.path
    if root and path.startswith(root + "/"):
        path = path[len(root) + 1:]
    return path

def _path(file):
    return file.path

def protobuf_shard_action(ctx, proto_info, protoc, windows, package, stem, extract_tool):
    """Runs protoc and the extractor for one proto_library.

    Args:
      ctx: The rule or aspect context.
      proto_info: The ProtoInfo of the documented proto_library.
      protoc: The FilesToRunProvider of protoc, from the proto toolchain.
      windows: Whether the execution platform is Windows, which joins paths with ';'.
      package: Package segment of the shard's symbol ids.
      stem: Output name prefix, relative to the target's package.
      extract_tool: The DocIr extractor executable.

    Returns:
      A struct with the DocIr `shard` and the `descriptor` set file.
    """
    root = proto_info.proto_source_root
    sources = proto_info.direct_sources
    if not sources:
        fail("protobuf_doc_shard " + str(ctx.label) + ": proto_library lists no .proto srcs")

    # The library's own descriptor set is left out: protoc drops the comments of a file it
    # reads from a descriptor set, and the docs come from the comments.
    own_set = proto_info.direct_descriptor_set
    dep_sets = [f for f in proto_info.transitive_descriptor_sets.to_list() if f != own_set]

    descriptor = ctx.actions.declare_file(stem + ".descriptor.pb")
    names = [_root_relative(src, root) for src in sources]
    args = ctx.actions.args()
    args.add("--proto_path=" + (root if root else "."))
    args.add_joined("--descriptor_set_in", dep_sets, join_with = ";" if windows else ":", map_each = _path, omit_if_empty = True)
    args.add("--include_source_info")
    args.add("--descriptor_set_out", descriptor)
    args.add_all(sources)
    ctx.actions.run(
        executable = protoc,
        arguments = [args],
        inputs = depset(sources, transitive = [depset(dep_sets)]),
        outputs = [descriptor],
        mnemonic = "DxDocsProtoc",
        toolchain = None,
        progress_message = "Reading Protobuf docs for %{label}",
    )

    shard = ctx.actions.declare_file(stem + ".ir.pb")
    extract = ctx.actions.args()
    extract.add("--descriptor", descriptor)
    extract.add("--package", package)
    extract.add("--out", shard)
    for name, src in zip(names, sources):
        extract.add("--src", name + "=" + repo_relative(src.short_path))
        if not src.is_source:
            extract.add("--generated", name)
    ctx.actions.run(
        executable = extract_tool,
        arguments = [extract],
        inputs = [descriptor],
        outputs = [shard],
        mnemonic = "DxDocsProtobufExtract",
        toolchain = None,
        progress_message = "Extracting Protobuf docs for %{label}",
    )
    return struct(shard = shard, descriptor = descriptor)

def _protobuf_doc_shard_impl(ctx):
    toolchain = ctx.toolchains[PROTO_TOOLCHAIN]
    if toolchain == None:
        fail("protobuf_doc_shard " + str(ctx.label) + ": no protobuf toolchain is registered for this platform")
    package = ctx.attr.package or ctx.attr.proto.label.name
    action = protobuf_shard_action(
        ctx,
        ctx.attr.proto[ProtoInfo],
        toolchain.proto.proto_compiler,
        ctx.attr._exec_os[ExecOsInfo].windows,
        package,
        ctx.label.name,
        ctx.executable._extract,
    )
    return [
        DefaultInfo(files = depset([action.shard])),
        OutputGroupInfo(descriptor_set = depset([action.descriptor])),
        DocsShardInfo(language = "protobuf", package = package, shard = action.shard),
    ]

docs_protobuf_shard = rule(
    doc = "Converts one proto_library's messages, enums and services into a DocIr shard with protoc.",
    implementation = _protobuf_doc_shard_impl,
    attrs = {
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the name of proto."),
        "proto": attr.label(
            mandatory = True,
            providers = [ProtoInfo],
            doc = "The documented proto_library; its own .proto srcs are documented.",
        ),
        "_exec_os": attr.label(
            default = Label("//libs/exec_os"),
            cfg = "exec",
            providers = [ExecOsInfo],
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/protobuf/extract:protobuf_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
    toolchains = [config_common.toolchain_type(PROTO_TOOLCHAIN, mandatory = False)],
)

def protobuf_doc_shard(name, proto, package = "", **kwargs):
    """Declares the DocIr shard of one proto_library.

    Args:
      name: Name of the shard target.
      proto: The documented proto_library.
      package: Package segment of every symbol id; defaults to the name of proto.
      **kwargs: Common attributes such as tags and visibility.
    """
    docs_protobuf_shard(
        name = name,
        proto = proto,
        package = package,
        **kwargs
    )
