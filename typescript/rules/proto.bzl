"""Runs protoc with protoc-gen-es over one proto_library."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")
load("//libs/exec_os:defs.bzl", "ExecOsInfo")

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

def _out_dir(ctx, info):
    """Returns the protoc output directory that matches the sibling-declared files."""
    root = info.proto_source_root
    bin_dir = ctx.bin_dir.path
    if root.startswith(bin_dir):
        root = root[len(bin_dir):].removeprefix("/")
    if root in ("", "."):
        return bin_dir
    return bin_dir + "/" + root

def _check_package(ctx, info):
    for src in info.direct_sources:
        if src.owner.repo_name != ctx.label.repo_name or src.owner.package != ctx.label.package:
            fail("%s must be in the package of its proto_library %s" % (ctx.label, ctx.attr.proto.label))

def _ts_proto_srcs_impl(ctx):
    proto_info = ctx.attr.proto[ProtoInfo]
    _check_package(ctx, proto_info)
    protos = _es_files(proto_info)
    outs = [ctx.actions.declare_file(src.basename[:-len(".proto")] + "_pb.ts", sibling = src) for src in proto_info.direct_sources]
    plugin = ctx.executable._plugin
    descriptors = proto_info.transitive_descriptor_sets
    args = ctx.actions.args()
    args.add("--plugin=protoc-gen-es=" + plugin.path)
    args.add("--es_out=target=ts:" + _out_dir(ctx, proto_info))
    args.add_joined("--descriptor_set_in", descriptors, join_with = ";" if ctx.attr._exec_os[ExecOsInfo].windows else ":", format_each = "%s", omit_if_empty = True, map_each = _path)
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
        "_exec_os": attr.label(
            default = "//libs/exec_os",
            cfg = "exec",
            providers = [ExecOsInfo],
        ),
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
