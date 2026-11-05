"""Runs protoc with its C# generator over one proto_library."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")

def pascal_case(name):
    """Returns name the way protoc spells a C# file: a_b_c becomes ABC, with a letter after a digit capitalised too."""
    out = ""
    upper = True
    for ch in name.elems():
        if ch == "_":
            upper = True
        elif upper:
            out += ch.upper()
            upper = False
        else:
            out += ch
            upper = ch.isdigit()
    return out

def _paths(proto_info):
    """Returns the proto path, relative to the import root, of each direct source."""
    root = proto_info.proto_source_root
    paths = []
    for src in proto_info.direct_sources:
        path = src.path
        if root and path.startswith(root + "/"):
            path = path[len(root) + 1:]
        paths.append(path)
    return paths

def _path(file):
    return file.path

def _csharp_proto_srcs_impl(ctx):
    proto_info = ctx.attr.proto[ProtoInfo]
    protos = _paths(proto_info)
    outs = [ctx.actions.declare_file(ctx.label.name + "/" + pascal_case(path.rsplit("/", 1)[-1][:-len(".proto")]) + ".cs") for path in protos]
    descriptors = proto_info.transitive_descriptor_sets
    args = ctx.actions.args()
    args.add("--csharp_out=" + "/".join([ctx.bin_dir.path, ctx.label.package, ctx.label.name]).replace("//", "/"))
    args.add_joined("--descriptor_set_in", descriptors, join_with = ":", format_each = "%s", omit_if_empty = True, map_each = _path)
    args.add_all(protos)
    ctx.actions.run(
        executable = ctx.executable._protoc,
        arguments = [args],
        inputs = descriptors,
        outputs = outs,
        mnemonic = "CsharpProto",
        progress_message = "Generating C# for %{label}",
    )
    return [DefaultInfo(files = depset(outs))]

csharp_proto_srcs = rule(
    implementation = _csharp_proto_srcs_impl,
    attrs = {
        "proto": attr.label(mandatory = True, providers = [ProtoInfo]),
        "_protoc": attr.label(
            default = "@protobuf//:protoc",
            executable = True,
            cfg = "exec",
        ),
    },
    doc = "Generates the <Name>.cs files of one proto_library.",
)
