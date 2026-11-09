"""Swift wrappers that build for Linux and macOS."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftInfo")
load("@rules_swift//swift:swift_binary.bzl", _swift_binary = "swift_binary")
load("@rules_swift//swift:swift_interop_hint.bzl", _swift_interop_hint = "swift_interop_hint")
load("@rules_swift//swift:swift_library.bzl", _swift_library = "swift_library")
load("@rules_swift//swift:swift_test.bzl", _swift_test = "swift_test")
load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("//libs/starlark:fuzz.bzl", "dx_fuzz_binary", "dx_fuzz_runner")
load("//libs/starlark:fuzz_bench.bzl", "merge_tags", "split_runner_kwargs", "split_test_kwargs", "with_default_size")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//swift/platforms:transition.bzl", "swift_linux_platforms", "swift_linux_transition")

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
_STATIC = ["swift.static_stdlib", "-static_link_cpp_runtimes"]
_INDEX = ["swift.index_while_building", "swift.disable_system_index", "-swift.index_store_compress"]
_SPECS = [("swift", "swift")]
SWIFT_QUALITY_TOOLS = {
    "swift_format": Label("//swift/tools:swift_format"),
    "swift_lint": Label("//swift/tools:swift_format"),
}
_EXEC_PROVIDES = DX_EXEC_PROVIDES + [QualityToolsInfo]

_swift_library_forward = dx_library_forward_rule(
    provides = [SwiftInfo, CcInfo, DefaultInfo, QualitySourcesInfo, QualityToolsInfo],
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

_swift_forward_test = dx_executable_forward_rule(
    kind = "test",
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

def swift_library(name, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_library and builds it for Linux, or with Xcode on macOS."""
    dx_wrap(name, _swift_library, _swift_library_forward, srcs, visibility = visibility, upstream_kwargs = dx_append_flags(kwargs, "features", _STATIC + _INDEX), quality_srcs = srcs or [], extra_forward_kwargs = {"platforms": platforms if platforms != None else swift_linux_platforms()}, **kwargs)

def swift_binary(name, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_binary and builds it for Linux, or with Xcode on macOS."""
    dx_wrap_binary(name, _swift_binary, _swift_binary_forward, srcs, visibility = visibility, upstream_kwargs = dx_append_flags(kwargs, "features", _STATIC + _INDEX), quality_srcs = srcs or [], extra_forward_kwargs = {"platforms": platforms if platforms != None else swift_linux_platforms()}, **kwargs)

def swift_test(name, srcs = None, visibility = None, platforms = None, **kwargs):
    """Wraps swift_test and builds it for Linux, or with Xcode on macOS."""
    dx_wrap_test(name, _swift_test, _swift_forward_test, srcs, visibility = visibility, upstream_kwargs = dx_append_flags(kwargs, "features", _STATIC + _INDEX), quality_srcs = srcs or [], extra_forward_kwargs = {"platforms": platforms if platforms != None else swift_linux_platforms()}, **kwargs)

def swift_fuzz_test(name, srcs = None, visibility = None, tags = None, copts = None, linkopts = None, **kwargs):
    """A libFuzzer binary with AddressSanitizer plus a `<name>_run` runner for `dx fuzz`. Linux only."""
    build, run = split_runner_kwargs(kwargs)
    swift_binary(
        name = name + "_bin",
        srcs = srcs,
        visibility = ["//visibility:private"],
        testonly = True,
        tags = merge_tags(tags, "manual"),
        copts = ["-sanitize=fuzzer,address"] + (copts or []),
        linkopts = ["-fsanitize=address"] + (linkopts or []),
        deps = build.pop("deps", []) + [Label("//swift/toolchain:libfuzzer")],
        features = build.pop("features", []) + ["swift.no_entry_point_rename"],
        target_compatible_with = ["@platforms//os:linux"],
        **build
    )
    dx_fuzz_binary(
        name = name,
        binary = ":" + name + "_bin",
        pinned_libfuzzer = False,
        visibility = visibility,
        testonly = True,
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        **run
    )
    dx_fuzz_runner(name, ":" + name, visibility = visibility)

def swift_bench(name, *, srcs = None, visibility = None, tags = None, **kwargs):
    """A swift_binary timing loop run by `dx bench`; it links `//swift/bench` to write dx-bench.json."""
    binary = name + "_bin"
    build, run = split_test_kwargs(kwargs)
    run = with_default_size(run)
    build["deps"] = build.get("deps", []) + [Label("//swift/bench")]
    swift_binary(
        name = binary,
        srcs = srcs,
        visibility = ["//visibility:private"],
        tags = merge_tags(tags, "manual"),
        testonly = True,
        **build
    )
    native_test(
        name = name,
        src = ":" + binary,
        tags = merge_tags(tags, "dx-bench", "manual"),
        testonly = True,
        visibility = visibility,
        **run
    )
