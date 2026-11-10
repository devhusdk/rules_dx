"""Swift wrappers that build for Linux and macOS."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftInfo", "SwiftSymbolGraphInfo")
load("@rules_swift//swift:swift_binary.bzl", _swift_binary = "swift_binary")
load("@rules_swift//swift:swift_interop_hint.bzl", _swift_interop_hint = "swift_interop_hint")
load("@rules_swift//swift:swift_library.bzl", _swift_library = "swift_library")
load("@rules_swift//swift:swift_symbol_graph_aspect.bzl", "swift_symbol_graph_aspect")
load("@rules_swift//swift:swift_test.bzl", _swift_test = "swift_test")
load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "dx_bench_test")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_warnings_flags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core:native_coverage.bzl", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//swift/private/platforms:transition.bzl", "swift_linux_platforms", "swift_linux_transition")
load("//swift/private:providers.bzl", "SwiftDocsInfo")

# Re-exported unchanged: BUILD files need it to expose C modules to Swift.
swift_interop_hint = _swift_interop_hint

_PLATFORM_ATTR = {
    "platforms": attr.label_list(
        doc = "The one platform to build for. Empty uses the current target platform.",
        providers = [platform_common.PlatformInfo],
    ),
    "_swift_extra_toolchains": attr.string_list(
        default = [str(Label("@local_config_apple_cc_toolchains//:all"))],
    ),
}
_WINDOWS_RUNTIME = Label("@dx_swift_windows_x86_64//:runtime_dlls")
_WINDOWS_RUNTIME_ROOT = "..\\" + _WINDOWS_RUNTIME.workspace_root.removeprefix("external/")
SWIFT_WINDOWS_RUNTIME_PATH = _WINDOWS_RUNTIME_ROOT + "\\runtime\\usr\\bin;" + _WINDOWS_RUNTIME_ROOT + "\\platform\\Developer\\Library\\XCTest-6.4.1\\usr\\bin64"
SWIFT_WINDOWS_RUNTIME_DLLS = _WINDOWS_RUNTIME
_STATIC = ["swift.static_stdlib", "-static_link_cpp_runtimes"]
_INDEX = ["swift.index_while_building", "swift.disable_system_index", "-swift.index_store_compress"]
_SPECS = [("swift", "swift")]
SWIFT_QUALITY_TOOLS = {
    "swift_format": Label("//swift/private/tools:swift_format"),
    "swift_lint": Label("//swift/private/tools:swift_format"),
}
_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

def _swift_docs_providers(ctx, upstream):
    graphs = upstream[SwiftSymbolGraphInfo].direct_symbol_graphs if SwiftSymbolGraphInfo in upstream else []
    return [SwiftDocsInfo(graphs = graphs, srcs = [f for f in ctx.files.srcs if f.extension == "swift"])]

_swift_library_forward = dx_library_forward_rule(
    provides = [SwiftInfo, CcInfo, DefaultInfo, QualitySourcesInfo, QualityToolsInfo, SwiftDocsInfo],
    extra_providers = _swift_docs_providers,
    upstream_aspects = [swift_symbol_graph_aspect],
    required_providers = [(SwiftInfo, "SwiftInfo"), (CcInfo, "CcInfo")],
    quality_specs = _SPECS,
    what = "swift_*",
    allow_files = True,
    upstream_providers = [[SwiftInfo]],
    extra_attrs = _PLATFORM_ATTR,
    runtime = "besteffort",
    upstream_cfg = swift_linux_transition,
    quality_tools = SWIFT_QUALITY_TOOLS,
)

_swift_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "swift_*",
    allow_files = True,
    upstream_providers = None,
    extra_attrs = _PLATFORM_ATTR,
    runtime = "besteffort",
    upstream_cfg = swift_linux_transition,
    quality_tools = SWIFT_QUALITY_TOOLS,
)

def _swift_test_coverage(ctx, upstream):
    return dx_native_test_coverage(ctx, upstream, llvm_cov = ctx.executable._swift_llvm_cov, llvm_profdata = ctx.executable._swift_llvm_profdata)

def _swift_tool_attr(label):
    return attr.label(default = Label(label), executable = True, allow_files = True, cfg = config.exec(exec_group = "test"))

_swift_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "swift_*",
    allow_files = True,
    upstream_providers = None,
    extra_attrs = _PLATFORM_ATTR | dx_lcov_merger_attr() | dx_native_coverage_attrs() | {
        "_swift_llvm_cov": _swift_tool_attr("//swift/private/tools:llvm_cov"),
        "_swift_llvm_profdata": _swift_tool_attr("//swift/private/tools:llvm_profdata"),
    },
    runtime = "besteffort",
    upstream_cfg = swift_linux_transition,
    quality_tools = SWIFT_QUALITY_TOOLS,
    coverage_providers = _swift_test_coverage,
    toolchains = dx_native_coverage_toolchains(),
)

def _swift_upstream_kwargs(kwargs):
    """Returns the swift upstream kwargs with the static, index and warnings-as-errors settings."""
    return dx_append_flags(dx_warnings_flags(kwargs, "copts", ["-warnings-as-errors"]), "features", _STATIC + _INDEX)

def swift_library(name, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_library and builds it for Linux, or with Xcode on macOS."""
    dx_wrap(name, _swift_library, _swift_library_forward, srcs, visibility = visibility, upstream_kwargs = _swift_upstream_kwargs(kwargs), quality_srcs = srcs or [], extra_forward_kwargs = {"platforms": platforms if platforms != None else swift_linux_platforms()}, **kwargs)

def swift_binary(name, *, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_binary and builds it for Linux, or with Xcode on macOS."""
    dx_wrap_binary(name, _swift_binary, _swift_binary_forward, srcs, visibility = visibility, upstream_kwargs = _swift_upstream_kwargs(kwargs), quality_srcs = srcs or [], extra_forward_kwargs = {"platforms": platforms if platforms != None else swift_linux_platforms()}, **kwargs)

def _swift_runtime_kwargs(kwargs, platforms):
    """Returns the forwarder kwargs that ship the sysroot's shared libraries with a swift_test and point the loader at them, so no host library is needed."""
    env = dict(kwargs.get("env", {}))
    env.setdefault("LD_LIBRARY_PATH", "")
    out = {"platforms": platforms if platforms != None else swift_linux_platforms()}
    out["data"] = dx_append_flags(kwargs, "data", [Label("//swift/private/toolchain:runtime_libs")] + select({
        Label("//swift/private/platforms:is_windows_x86_64"): [SWIFT_WINDOWS_RUNTIME_DLLS],
        "//conditions:default": [],
    }))["data"]
    out["env"] = select({
        Label("//swift/private/platforms:is_linux_aarch64"): dict(env, LD_LIBRARY_PATH = "../bazel_linux_packages++apt+dx_swift_sysroot_linux_aarch64/host_libs"),
        Label("//swift/private/platforms:is_linux_x86_64"): dict(env, LD_LIBRARY_PATH = "../bazel_linux_packages++apt+dx_swift_sysroot_linux_x86_64/host_libs"),
        Label("//swift/private/platforms:is_windows_x86_64"): dict(env, PATH = SWIFT_WINDOWS_RUNTIME_PATH),
        "//conditions:default": env,
    })
    return out

def _swift_coverage_kwargs(kwargs):
    """Links Swift's own profile runtime into a swift_test under bazel coverage."""
    out = _swift_upstream_kwargs(kwargs)
    out = dx_append_flags(out, "features", select({
        Label("//swift/private:coverage_linux"): ["-llvm_coverage_map_format"],
        "//conditions:default": [],
    }))
    return dx_append_flags(out, "deps", select({
        Label("//swift/private:coverage_linux"): [Label("//swift/private/toolchain:profile_runtime")],
        "//conditions:default": [],
    }))

def swift_test(name, *, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_test and builds it for Linux, or with Xcode on macOS."""
    dx_wrap_test(name, _swift_test, _swift_forward_test, srcs, visibility = visibility, upstream_kwargs = _swift_coverage_kwargs(kwargs), quality_srcs = srcs or [], extra_forward_kwargs = _swift_runtime_kwargs(kwargs, platforms), **kwargs)

def swift_fuzz_test(name, *, srcs = None, visibility = None, tags = None, copts = None, linkopts = None, **kwargs):
    """A libFuzzer binary with AddressSanitizer plus a `<name>_run` runner for `dx fuzz`. Linux only."""
    binary_kwargs = {
        "srcs": srcs,
        "copts": ["-sanitize=fuzzer,address"] + (copts or []),
        "linkopts": ["-fsanitize=address"] + (linkopts or []),
        "deps": kwargs.pop("deps", []) + [Label("//swift/private/toolchain:libfuzzer")],
        "features": kwargs.pop("features", []) + ["swift.no_entry_point_rename"],
        "target_compatible_with": ["@platforms//os:linux"],
    }
    dx_fuzz_test(name, swift_binary, binary_kwargs, tags = tags, visibility = visibility, pinned_libfuzzer = False, **kwargs)

def swift_bench(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """A swift_binary timing loop run by `dx bench`; it links `//swift/bench` to write dx-bench.json."""
    dx_bench_test(
        name,
        swift_binary,
        native_test,
        kwargs,
        tags = tags,
        visibility = visibility,
        binary_kwargs = {"srcs": srcs},
        extra_deps = [Label("//swift/bench")],
        runner_kwargs = {"src": ":" + name + "_bin"},
    )
