"""Python gRPC stubs from a proto_library."""

load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")
load("//cli/exec_os:defs.bzl", "ExecOsInfo")
load(":defs.bzl", "python_binary", "python_library")

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

def _srcs_impl(ctx):
    info = ctx.attr.proto[ProtoInfo]
    _check_package(ctx, info)
    root = info.proto_source_root
    outs = []
    names = []
    for src in info.direct_sources:
        path = src.path
        if root != "." and path.startswith(root + "/"):
            path = path[len(root) + 1:]
        names.append(path)
        outs.append(ctx.actions.declare_file(src.basename[:-len(".proto")] + "_pb2_grpc.py", sibling = src))

    args = ctx.actions.args()
    args.add_joined(
        info.transitive_descriptor_sets,
        join_with = ";" if ctx.attr._exec_os[ExecOsInfo].windows else ":",
        format_joined = "--descriptor_set_in=%s",
    )
    args.add("--grpc_python_out=" + _out_dir(ctx, info))
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
    doc = "Compiles the gRPC services of a proto_library into Python stubs.",
    attrs = {
        "proto": attr.label(mandatory = True, providers = [ProtoInfo], doc = "The proto_library with the service."),
        "tool": attr.label(mandatory = True, executable = True, cfg = "exec", doc = "The protoc binary from grpcio-tools."),
        "_exec_os": attr.label(
            default = "//cli/exec_os",
            cfg = "exec",
            providers = [ExecOsInfo],
        ),
    },
)

def python_grpc_library(name, proto, py_proto, grpc_tools = "@pypi//grpcio_tools", grpc = "@pypi//grpcio", dep_group = None, **kwargs):
    """Compiles the services of proto into <name>_pb2_grpc.py."""
    python_binary(
        name = name + "_protoc",
        srcs = ["@rules_dx//python/private:grpc_protoc.py"],
        main = "@rules_dx//python/private:grpc_protoc.py",
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
