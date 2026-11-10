"""Minimal Python wrappers."""

load("@bazel_skylib//rules:native_binary.bzl", "native_binary")
load("@protobuf//bazel/common:proto_info.bzl", "ProtoInfo")
load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo", _PyWheelsInfo = "PyWheelsInfo", _py_binary = "py_binary", _py_library = "py_library", _py_pytest_test = "py_pytest_test", _py_test = "py_test")
load("//cli/exec_os:defs.bzl", "ExecOsInfo")
load("//core:entry.bzl", "dx_entry_quality_srcs")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_PY_LIBRARY_PROVIDES = [
    _PyInfo,
    _PyWheelsInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_PY_BINARY_PROVIDES = [
    _PyInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_PY_SOURCE_SPECS = [("python", "py"), ("python_stub", "pyi")]
_DX_PY_SOURCE_EXTS = [".py", ".pyi"]

_python_library_forward = dx_library_forward_rule(
    provides = _DX_PY_LIBRARY_PROVIDES,
    required_providers = [(_PyInfo, "PyInfo"), (_PyWheelsInfo, "PyWheelsInfo")],
    quality_specs = _DX_PY_SOURCE_SPECS,
    what = "python_*",
    allow_files = _DX_PY_SOURCE_EXTS,
    upstream_providers = [[_PyInfo]],
)

_python_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_PY_BINARY_PROVIDES,
    required_providers = [(_PyInfo, "PyInfo")],
    quality_specs = _DX_PY_SOURCE_SPECS,
    what = "python_*",
    allow_files = _DX_PY_SOURCE_EXTS,
    upstream_providers = [[_PyInfo]],
)

_python_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_PY_BINARY_PROVIDES,
    required_providers = [(_PyInfo, "PyInfo")],
    quality_specs = _DX_PY_SOURCE_SPECS,
    what = "python_*",
    allow_files = _DX_PY_SOURCE_EXTS,
    upstream_providers = [[_PyInfo]],
    extra_attrs = dx_lcov_merger_attr(),
)

def _python_main_matches(name, srcs):
    """Returns the srcs that end in the wrapper basename plus .py."""
    proposed = name.split("/")[-1] + ".py"
    return [src for src in srcs if str(src) == proposed or str(src).endswith(":" + proposed) or str(src).endswith("/" + proposed)]

def python_main(name, srcs):
    """Returns the src py_binary picks as main for the wrapper name, or None."""
    if type(srcs) != "list" or len(srcs) <= 1:
        return None
    matches = _python_main_matches(name, srcs)
    return matches[0] if len(matches) == 1 else None

def python_main_rejection(name, srcs):
    """Returns the rejection for multiple srcs without one main match, or None."""
    if type(srcs) != "list" or len(srcs) <= 1:
        return None
    proposed = name.split("/")[-1] + ".py"
    matches = _python_main_matches(name, srcs)
    if not matches:
        return "has multiple srcs ({}) and none is {}; set main.".format(", ".join([str(s) for s in srcs]), proposed)
    if len(matches) > 1:
        return "has multiple srcs matching {}: {}; set main.".format(proposed, ", ".join([str(m) for m in matches]))
    return None

def _python_main_kwargs(kind, name, srcs, kwargs):
    """Returns upstream kwargs with main set from the wrapper name, or None."""
    if kwargs.get("main") != None:
        return None
    rejection = python_main_rejection(name, srcs)
    if rejection != None:
        fail("{} //{}:{} {}".format(kind, native.package_name(), name, rejection))
    main = python_main(name, srcs)
    if main == None:
        return None
    return dict(kwargs, main = main)

def python_library(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over py_library."""
    dx_wrap(name, _py_library, _python_library_forward, srcs, visibility = visibility, **kwargs)

def python_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Minimal wrapper over py_binary. main is name.py, else the single src; several srcs need a name.py match. main is linted unless it is an external label."""
    if main != None:
        kwargs["main"] = main
    srcs = srcs if srcs != None else []
    upstream_kwargs = _python_main_kwargs("python_binary", name, srcs, kwargs)
    dx_wrap_binary(name, _py_binary, _python_binary_forward, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, quality_srcs = dx_entry_quality_srcs(main if main != None else python_main(name, srcs), srcs), **kwargs)

def python_test_rejection(runner, kwargs):
    """Returns the rejection for a bad python_test runner or kwargs, or None."""
    if runner not in ("pytest", "plain"):
        return "python_test runner must be \"pytest\" or \"plain\", got %r." % (runner,)
    if runner == "pytest" and "main" in kwargs:
        return "python_test with runner = \"pytest\" runs pytest; set runner = \"plain\" to use main."
    return None

def python_test(name, *, srcs = None, main = None, runner = "pytest", visibility = None, **kwargs):
    """Wrapper over py_pytest_test, or py_test when runner = "plain". runner defaults to "pytest"; plain infers main as python_binary does."""
    if main != None:
        kwargs["main"] = main
    rejection = python_test_rejection(runner, kwargs)
    if rejection != None:
        fail(rejection)
    if runner == "plain":
        upstream_kwargs = _python_main_kwargs("python_test", name, srcs, kwargs)
        dx_wrap_test(name, _py_test, _python_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, quality_srcs = dx_entry_quality_srcs(main if main != None else python_main(name, srcs), srcs), **kwargs)
    else:
        dx_wrap_test(name, _py_pytest_test, _python_forward_test, srcs, visibility = visibility, **kwargs)

def python_fuzz_test(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """A python_binary that calls atheris plus a `<name>_run` runner for `dx fuzz`. Needs the atheris wheel: linux x86_64 only."""
    kwargs = dict(kwargs)
    linux_x86_64 = ["@platforms//os:linux", "@platforms//cpu:x86_64"]
    compatible = linux_x86_64 + [c for c in kwargs.pop("target_compatible_with", []) if c not in linux_x86_64]
    dx_fuzz_test(
        name,
        python_binary,
        {"srcs": srcs, "target_compatible_with": compatible},
        tags = tags,
        visibility = visibility,
        fuzz_rule = native_binary,
        fuzz_kwargs = {"src": ":" + name + "_bin", "out": name + "_fuzz", "target_compatible_with": compatible},
        **kwargs
    )

def python_bench(name, *, srcs = None, args = None, tags = None, deps = None, **kwargs):
    """python_test run by dx bench: pytest-benchmark numbers land in dx-bench.json."""
    python_test(
        name = name,
        srcs = srcs,
        args = ["-p", "dx_bench_plugin", "--benchmark-only"] + (args or []),
        tags = merge_tags(tags, "dx-bench", "manual"),
        deps = (deps or []) + [Label("//python/private:bench_plugin")],
        **with_default_size(kwargs)
    )

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
