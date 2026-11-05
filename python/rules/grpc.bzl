"""py_grpc_library: Python gRPC stubs for a proto_library."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")
load("//python/rules:defs.bzl", "python_binary", "python_library")

def _srcs_impl(ctx):
    info = ctx.attr.proto[ProtoInfo]
    root = info.proto_source_root
    outs = []
    names = []
    for src in info.direct_sources:
        path = src.path
        if root != "." and path.startswith(root + "/"):
            path = path[len(root) + 1:]
        names.append(path)
        outs.append(ctx.actions.declare_file(src.basename[:-len(".proto")] + "_pb2_grpc.py"))

    windows = ctx.target_platform_has_constraint(ctx.attr._windows[platform_common.ConstraintValueInfo])
    args = ctx.actions.args()
    args.add_joined(
        info.transitive_descriptor_sets,
        join_with = ";" if windows else ":",
        format_joined = "--descriptor_set_in=%s",
    )
    args.add("--grpc_python_out=" + ctx.bin_dir.path)
    args.add_all(names)
    ctx.actions.run(
        executable = ctx.executable.tool,
        arguments = [args],
        inputs = info.transitive_descriptor_sets,
        outputs = outs,
        mnemonic = "PyGrpcProtoc",
        progress_message = "Generating Python gRPC stubs for %{label}",
    )
    return [DefaultInfo(files = depset(outs))]

_py_grpc_srcs = rule(
    implementation = _srcs_impl,
    attrs = {
        "proto": attr.label(mandatory = True, providers = [ProtoInfo]),
        "tool": attr.label(mandatory = True, executable = True, cfg = "exec"),
        "_windows": attr.label(default = "@platforms//os:windows"),
    },
)

def py_grpc_library(name, proto, py_proto, grpc_tools = "@pypi//grpcio_tools", grpc = "@pypi//grpcio", dep_group = None, **kwargs):
    """Compiles the services of proto into <name>_pb2_grpc.py.

    Args:
      name: target name of the py_library.
      proto: the proto_library with the service.
      py_proto: the py_proto_library for the same proto_library.
      grpc_tools: label of the grpcio-tools package that provides protoc.
      grpc: label of the grpcio package the stubs import.
      dep_group: uv project name for the packages, when the hub has several.
      **kwargs: forwarded to the py_library, for example visibility.
    """
    python_binary(
        name = name + "_protoc",
        srcs = ["@rules_dx//python/rules:grpc_protoc.py"],
        main = "@rules_dx//python/rules:grpc_protoc.py",
        deps = [grpc_tools],
        dep_group = dep_group,
        tags = ["manual"],
        visibility = ["//visibility:private"],
        testonly = kwargs.get("testonly", False),
    )
    _py_grpc_srcs(
        name = name + "_srcs",
        proto = proto,
        tool = ":" + name + "_protoc",
        tags = ["manual"],
        testonly = kwargs.get("testonly", False),
    )
    python_library(
        name = name,
        srcs = [":" + name + "_srcs"],
        deps = [py_proto, grpc],
        **kwargs
    )
