"""Swift wrappers that build for Linux and macOS."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftInfo")
load("@rules_swift//swift:swift_binary.bzl", _swift_binary = "swift_binary")
load("@rules_swift//swift:swift_interop_hint.bzl", _swift_interop_hint = "swift_interop_hint")
load("@rules_swift//swift:swift_library.bzl", _swift_library = "swift_library")
load("@rules_swift//swift:swift_test.bzl", _swift_test = "swift_test")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_append_flags", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo")
load("//swift/platforms:transition.bzl", "swift_extra_toolchains", "swift_linux_platform", "swift_linux_transition")

swift_interop_hint = _swift_interop_hint

_PLATFORM_ATTR = {
    "swift_extra_toolchains": attr.string_list(doc = "Toolchains put first for Swift targets."),
    "swift_platform": attr.string(doc = "Swift platform to build for."),
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

def _forward():
    return {"swift_extra_toolchains": swift_extra_toolchains(), "swift_platform": swift_linux_platform()}

def swift_library(name, srcs = None, visibility = None, **kwargs):
    """Wraps swift_library and builds it for Linux, or with Xcode on macOS."""
    dx_wrap(name, _swift_library, _swift_library_forward, srcs, visibility = visibility, upstream_kwargs = dx_append_flags(kwargs, "features", _STATIC + _INDEX), quality_srcs = srcs or [], extra_forward_kwargs = _forward(), **kwargs)

def swift_binary(name, srcs = None, visibility = None, **kwargs):
    """Wraps swift_binary and builds it for Linux, or with Xcode on macOS."""
    dx_wrap_binary(name, _swift_binary, _swift_binary_forward, srcs, visibility = visibility, upstream_kwargs = dx_append_flags(kwargs, "features", _STATIC + _INDEX), quality_srcs = srcs or [], extra_forward_kwargs = _forward(), **kwargs)

def swift_test(name, srcs = None, visibility = None, **kwargs):
    """Wraps swift_test and builds it for Linux, or with Xcode on macOS."""
    dx_wrap_test(name, _swift_test, _swift_forward_test, srcs, visibility = visibility, upstream_kwargs = dx_append_flags(kwargs, "features", _STATIC + _INDEX), quality_srcs = srcs or [], extra_forward_kwargs = _forward(), **kwargs)
