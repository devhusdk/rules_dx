"""Swift wrappers that build for Linux and macOS."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftInfo", "SwiftSymbolGraphInfo")
load("@rules_swift//swift:swift_binary.bzl", _swift_binary = "swift_binary")
load("@rules_swift//swift:swift_interop_hint.bzl", _swift_interop_hint = "swift_interop_hint")
load("@rules_swift//swift:swift_library.bzl", _swift_library = "swift_library")
load("@rules_swift//swift:swift_library_group.bzl", _swift_library_group = "swift_library_group")
load("@rules_swift//swift:swift_symbol_graph_aspect.bzl", "swift_symbol_graph_aspect")
load("@rules_swift//swift:swift_test.bzl", _swift_test = "swift_test")
load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "dx_bench_test")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_language_kwargs", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_warnings_flags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core:msvc_crt.bzl", "dx_msvc_crt_reset_transition")
load("//core:platform.bzl", "platforms_transition")
load("//core:native_coverage.bzl", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage")
load("//core/quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//swift/private/platforms:support.bzl", _SWIFT_COMPATIBLE_WITH = "SWIFT_COMPATIBLE_WITH", _SWIFT_FUZZ_COMPATIBLE_WITH = "SWIFT_FUZZ_COMPATIBLE_WITH")
load("//swift/private/platforms:settings.bzl", "SWIFT_PLATFORM_SETTINGS", "swift_linux_platforms", "swift_platform_settings")
load("//swift/private:providers.bzl", "SwiftDocsInfo")

# Re-exported unchanged: BUILD files use them to expose C modules to Swift and to group libraries.
swift_interop_hint = _swift_interop_hint
swift_library_group = _swift_library_group

_PLATFORM_ATTR = {
    "platforms": attr.string_list(
        doc = "The one platform label to build for. Empty uses the command-line platform.",
    ),
    "_swift_extra_toolchains": attr.string_list(
        default = [str(Label("@local_config_apple_cc_toolchains//:all"))],
    ),
}
_swift_platforms_transition = platforms_transition(
    swift_platform_settings,
    inputs = SWIFT_PLATFORM_SETTINGS,
    outputs = SWIFT_PLATFORM_SETTINGS,
    single = "swift",
)
_RUNTIME_DLLS_ATTR = {
    "runtime_dlls": attr.label_list(
        doc = "The Swift runtime DLLs symlinked next to the executable. Set on windows_x86_64 only.",
        allow_files = [".dll"],
    ),
}
_WINDOWS_RUNTIME = Label("@dx_swift_windows_x86_64//:runtime_dlls")
SWIFT_WINDOWS_RUNTIME_DLLS = _WINDOWS_RUNTIME
SWIFT_COMPATIBLE_WITH = _SWIFT_COMPATIBLE_WITH
SWIFT_FUZZ_COMPATIBLE_WITH = _SWIFT_FUZZ_COMPATIBLE_WITH
_STATIC = ["swift.static_stdlib", "-static_link_cpp_runtimes"]
_INDEX = ["swift.index_while_building", "swift.disable_system_index", "-swift.index_store_compress"]
_SPECS = [("swift", "swift")]
SWIFT_QUALITY_TOOLS = {
    "swift_format": Label("//swift/private/tools:swift_format"),
    "swift_lint": Label("//swift/private/tools:swift_format"),
}
_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

def _swift_runtime_dlls(ctx):
    seen = {}
    for dll in ctx.files.runtime_dlls:
        seen[dll.basename] = dll
    return seen.values()

def _swift_docs_providers(ctx, upstream):
    graphs = upstream[SwiftSymbolGraphInfo].direct_symbol_graphs if SwiftSymbolGraphInfo in upstream else []
    return [SwiftDocsInfo(graphs = graphs, srcs = [f for f in ctx.files.srcs if f.extension == "swift"])]

_SWIFT = dx_language_spec(what = "swift_*", quality_specs = _SPECS, allow_files = True, quality_tools = SWIFT_QUALITY_TOOLS)

_swift_library_forward = dx_library_forward_rule(
    _SWIFT,
    provides = [SwiftInfo, CcInfo, DefaultInfo, QualitySourcesInfo, QualityToolsInfo, SwiftDocsInfo],
    extra_providers = _swift_docs_providers,
    upstream_aspects = [swift_symbol_graph_aspect],
    required_providers = [(SwiftInfo, "SwiftInfo"), (CcInfo, "CcInfo")],
    upstream_providers = [[SwiftInfo]],
    extra_attrs = _PLATFORM_ATTR,
    runtime = "besteffort",
    upstream_cfg = _swift_platforms_transition,
)

_swift_binary_forward = dx_executable_forward_rule(
    _SWIFT,
    kind = "executable",
    provides = _EXEC_PROVIDES,
    extra_attrs = _PLATFORM_ATTR | _RUNTIME_DLLS_ATTR,
    upstream_cfg = _swift_platforms_transition,
    cfg = dx_msvc_crt_reset_transition,
    sibling_files = _swift_runtime_dlls,
)

def _swift_test_coverage(ctx, upstream):
    tool_runfiles = ctx.runfiles().merge_all([
        ctx.attr._swift_llvm_cov[DefaultInfo].default_runfiles,
        ctx.attr._swift_llvm_profdata[DefaultInfo].default_runfiles,
    ])
    return dx_native_test_coverage(
        ctx,
        upstream,
        llvm_cov = ctx.executable._swift_llvm_cov,
        llvm_profdata = ctx.executable._swift_llvm_profdata,
        tool_runfiles = tool_runfiles,
    )

def _swift_tool_attr(label):
    return attr.label(default = Label(label), executable = True, allow_files = True, cfg = config.exec(exec_group = "test"))

_swift_forward_test = dx_executable_forward_rule(
    _SWIFT,
    kind = "test",
    provides = _EXEC_PROVIDES,
    extra_attrs = _PLATFORM_ATTR | _RUNTIME_DLLS_ATTR | dx_lcov_merger_attr() | dx_native_coverage_attrs() | {
        "_swift_llvm_cov": _swift_tool_attr("//swift/private/tools:llvm_cov"),
        "_swift_llvm_profdata": _swift_tool_attr("//swift/private/tools:llvm_profdata"),
    },
    upstream_cfg = _swift_platforms_transition,
    coverage_providers = _swift_test_coverage,
    toolchains = dx_native_coverage_toolchains(),
    cfg = dx_msvc_crt_reset_transition,
    sibling_files = _swift_runtime_dlls,
)

def swift_kwargs(kwargs):
    """Returns kwargs with the Swift platform guard appended."""
    return dx_language_kwargs(kwargs, [], SWIFT_COMPATIBLE_WITH)

def _swift_upstream_kwargs(kwargs):
    """Returns the swift upstream kwargs with the static, index and warnings-as-errors settings."""
    return dx_append_flags(dx_warnings_flags(kwargs, "copts", ["-warnings-as-errors"]), "features", _STATIC + _INDEX)

def swift_library(name, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_library and builds it for Linux, or with Xcode on macOS."""
    kwargs = swift_kwargs(kwargs)
    dx_wrap(name, _swift_library, _swift_library_forward, srcs, visibility = visibility, upstream_kwargs = _swift_upstream_kwargs(kwargs), quality_srcs = srcs or [], extra_forward_kwargs = {"platforms": platforms if platforms != None else swift_linux_platforms()}, **kwargs)

def swift_binary(name, *, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_binary and builds it for Linux, or with Xcode on macOS."""
    kwargs = swift_kwargs(kwargs)
    dx_wrap_binary(name, _swift_binary, _swift_binary_forward, srcs, visibility = visibility, upstream_kwargs = _swift_upstream_kwargs(kwargs), quality_srcs = srcs or [], extra_forward_kwargs = _swift_forward_kwargs(platforms), **kwargs)

def _swift_forward_kwargs(platforms):
    """Returns the forwarder kwargs naming the platform and the Swift runtime DLLs."""
    return {
        "platforms": platforms if platforms != None else swift_linux_platforms(),
        "runtime_dlls": select({
            Label("//swift/private/platforms:is_windows_x86_64"): [SWIFT_WINDOWS_RUNTIME_DLLS],
            "//conditions:default": [],
        }),
    }

def _swift_runtime_kwargs(kwargs, platforms):
    """Returns the forwarder kwargs that ship the sysroot's shared libraries with a swift_test, so no host library is needed."""
    out = _swift_forward_kwargs(platforms)
    out["data"] = dx_append_flags(kwargs, "data", [Label("//swift/private/toolchain:runtime_libs")])["data"]
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
    kwargs = swift_kwargs(kwargs)
    dx_wrap_test(name, _swift_test, _swift_forward_test, srcs, visibility = visibility, upstream_kwargs = _swift_coverage_kwargs(kwargs), quality_srcs = srcs or [], extra_forward_kwargs = _swift_runtime_kwargs(kwargs, platforms), **kwargs)

def swift_fuzz_test(name, *, srcs = None, visibility = None, tags = None, copts = None, linkopts = None, **kwargs):
    """A libFuzzer binary with AddressSanitizer plus a `<name>_run` runner for `dx fuzz`. Linux only."""
    binary_kwargs = {
        "srcs": srcs,
        "copts": ["-sanitize=fuzzer,address"] + (copts or []),
        "linkopts": linkopts or [],
        "deps": kwargs.pop("deps", []) + [Label("//swift/private/toolchain:libfuzzer"), Label("//swift/private/toolchain:asan_runtime")],
        "features": kwargs.pop("features", []) + ["swift.no_entry_point_rename"],
        "target_compatible_with": SWIFT_FUZZ_COMPATIBLE_WITH,
    }
    dx_fuzz_test(name, swift_binary, binary_kwargs, tags = tags, visibility = visibility, pinned_libfuzzer = False, **kwargs)

def swift_bench(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """A swift_binary timing loop run by `dx bench`; it links `//swift/private/bench` to write dx-bench.json."""
    dx_bench_test(
        name,
        swift_binary,
        native_test,
        kwargs,
        tags = tags,
        visibility = visibility,
        binary_kwargs = {"srcs": srcs},
        extra_deps = [Label("//swift/private/bench")],
        runner_kwargs = {"src": ":" + name + "_bin"},
    )
