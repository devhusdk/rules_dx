"""Runs protoc with protoc-gen-es over one proto_library."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")

def _es_files(proto_info):
    """Returns the proto path, relative to the import root, of each direct source."""
    root = proto_info.proto_source_root
    paths = []
    for src in proto_info.direct_sources:
        path = src.path
        if root and path.startswith(root + "/"):
            path = path[len(root) + 1:]
        paths.append(path)
    return paths

def _ts_proto_srcs_impl(ctx):
    proto_info = ctx.attr.proto[ProtoInfo]
    protos = _es_files(proto_info)
    outs = [ctx.actions.declare_file(path.rsplit("/", 1)[-1][:-len(".proto")] + "_pb.ts") for path in protos]
    plugin = ctx.executable._plugin
    descriptors = proto_info.transitive_descriptor_sets
    args = ctx.actions.args()
    args.add("--plugin=protoc-gen-es=" + plugin.path)
    args.add("--es_out=target=ts:" + ctx.bin_dir.path)
    args.add_joined("--descriptor_set_in", descriptors, join_with = ":", format_each = "%s", omit_if_empty = True, map_each = _path)
    args.add_all(protos)
    ctx.actions.run(
        executable = ctx.executable._protoc,
        arguments = [args],
        inputs = descriptors,
        tools = [ctx.attr._plugin[DefaultInfo].files_to_run],
        env = {"BAZEL_BINDIR": "."},
        outputs = outs,
        mnemonic = "TsProto",
        progress_message = "Generating TypeScript for %{label}",
    )
    return [DefaultInfo(files = depset(outs))]

def _path(file):
    return file.path

ts_proto_srcs = rule(
    implementation = _ts_proto_srcs_impl,
    attrs = {
        "proto": attr.label(mandatory = True, providers = [ProtoInfo]),
        "_protoc": attr.label(
            default = "@protobuf//:protoc",
            executable = True,
            cfg = "exec",
        ),
        "_plugin": attr.label(
            default = "//quality/tools/javascript/bin:protoc_gen_es",
            executable = True,
            cfg = "exec",
        ),
    },
    doc = "Generates the <name>_pb.ts modules of one proto_library.",
)
