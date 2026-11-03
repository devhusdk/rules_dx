"""Minimal C, C++ and Objective-C wrappers."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_library = "cc_library", _cc_test = "cc_test")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain")
load("@rules_cc//cc:objc_library.bzl", _objc_library = "objc_library")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//cc/toolchains:bindings.bzl", "cc_compiler_identity")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "UPSTREAM_CC_CLASS_EXTENSIONS")

_DX_CC_LIBRARY_PROVIDES = [
    CcInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CC_SOURCE_SPECS = [(k, UPSTREAM_CC_CLASS_EXTENSIONS[k]) for k in ("c", "cpp")]

_cc_library_forward = dx_library_forward_rule(
    provides = _DX_CC_LIBRARY_PROVIDES,
    required_providers = [(CcInfo, "CcInfo")],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = True,
    upstream_providers = [[CcInfo]],
    extra_attrs = {
        "hdrs": attr.label_list(
            doc = "Header files exported to dependent targets.",
            allow_files = True,
        ),
    },
    extra_quality_attrs = ["hdrs"],
)

_cc_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = True,
    upstream_providers = [[CcInfo]],
    optional_providers = [CcInfo],
    runtime = "besteffort",
)

_LLVM_COVERAGE_FEATURES = ["llvm_coverage_map_format", "-gcc_coverage_map_format"]

def cc_coverage_features(features, collect_code_coverage, compiler = ""):
    """Returns the features a cc_test builds with: LLVM coverage maps only under coverage and an LLVM or unset compiler."""
    if not collect_code_coverage:
        return features
    if compiler and cc_compiler_identity(compiler, "").coverage != "llvm":
        return features
    return features + [f for f in _LLVM_COVERAGE_FEATURES if f not in features]

def _cc_coverage_transition_impl(settings, _attr):
    features = cc_coverage_features(
        settings["//command_line_option:features"],
        settings["//command_line_option:collect_code_coverage"],
        settings["//command_line_option:compiler"] or "",
    )
    return {"//command_line_option:features": features}

_cc_coverage_transition = transition(
    implementation = _cc_coverage_transition_impl,
    inputs = ["//command_line_option:features", "//command_line_option:collect_code_coverage", "//command_line_option:compiler"],
    outputs = ["//command_line_option:features"],
)

def cc_coverage_unsupported(compiler):
    """Returns why native coverage cannot run on a known non-LLVM compiler, or an empty string."""
    if not compiler:
        return ""
    identity = cc_compiler_identity(compiler, "")
    if identity.family == "unknown" or identity.coverage == "llvm":
        return ""
    return "cc_test coverage needs an LLVM compiler (clang or clang-cl); the toolchain compiler is '%s'. Run without --collect_code_coverage or select an LLVM toolchain." % compiler

def _cc_test_coverage(ctx, upstream):
    toolchain = find_cc_toolchain(ctx, mandatory = False)
    reason = cc_coverage_unsupported(toolchain.compiler if toolchain else "")
    if reason:
        fail(reason)
    return dx_native_test_coverage(ctx, upstream)

CC_COVERAGE_TRANSITION = _cc_coverage_transition

_cc_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = True,
    upstream_providers = [[CcInfo]],
    extra_attrs = dx_lcov_merger_attr() | dx_native_coverage_attrs(),
    optional_providers = [CcInfo],
    upstream_cfg = _cc_coverage_transition,
    coverage_providers = _cc_test_coverage,
    toolchains = dx_native_coverage_toolchains(),
)

def cc_library(name, srcs = None, hdrs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_library."""
    effective_srcs = srcs if srcs != None else []
    effective_hdrs = hdrs if hdrs != None else []
    dx_wrap(name, _cc_library, _cc_library_forward, effective_srcs, visibility = visibility, hdrs = effective_hdrs, extra_forward_kwargs = {"hdrs": effective_hdrs}, **kwargs)

def objc_library(name, visibility = None, **kwargs):
    """The upstream objc_library under the caller's name, so rules_apple and rules_swift see it unchanged."""
    _objc_library(name = name, visibility = visibility, **kwargs)

def cc_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_binary that links Linux binaries with a build-id."""
    linkopts = kwargs.pop("linkopts", [])
    build_id = select({
        "@platforms//os:linux": ["-Wl,--build-id=sha1"],
        "//conditions:default": [],
    })
    dx_wrap_binary(name, _cc_binary, _cc_binary_forward, srcs, visibility = visibility, linkopts = build_id + linkopts, **kwargs)

def cc_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_test."""
    dx_wrap_test(name, _cc_test, _cc_forward_test, srcs, visibility = visibility, **kwargs)
