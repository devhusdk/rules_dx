"""Minimal CUDA wrappers that compile with the pinned nvcc."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_cuda//cuda:defs.bzl", _cuda_library = "cuda_library", _cuda_objects = "cuda_objects")
load("@rules_cuda//cuda/private:providers.bzl", "CudaInfo")
load("//cc/rules:defs.bzl", _cc_binary = "cc_binary", _cc_test = "cc_test")
load("//cuda/rules:components.bzl", _cuda_component_library = "cuda_component_library")
load("//cuda/toolchain:check.bzl", "cuda_check", "cuda_gpu_check")
load("//cuda/toolchain:settings.bzl", "CUDA_CELL_SETTINGS")
load("//cuda/toolchain:transition.bzl", "cuda_host_transition")
load("//libs/starlark:cells.bzl", "cells_compatible_with")
load("//libs/starlark:wrapper.bzl", "dx_library_forward_rule", "dx_upstream_tags")
load("//modules:cuda.bzl", "CUDA_CELLS", "CUDA_VERSION", "cuda_qualified_cells")
load("//quality:sources.bzl", "QualitySourcesInfo")

cuda_component_library = _cuda_component_library

CUDA_TARGET = cells_compatible_with(CUDA_CELLS, "//cuda/toolchain", cuda_qualified_cells(), settings = CUDA_CELL_SETTINGS)

_CUDA_SRCS = [".cu"]
_CUDA_HDRS = [".cuh", ".h", ".hh", ".hpp", ".hxx"]

_DX_CUDA_SOURCE_SPECS = [
    ("cuda", ["cu", "cuh"]),
    ("c", ["h"]),
    ("cpp", ["hh", "hpp", "hxx"]),
]

_cuda_library_forward = dx_library_forward_rule(
    provides = [
        CcInfo,
        CudaInfo,
        DefaultInfo,
        InstrumentedFilesInfo,
        QualitySourcesInfo,
    ],
    required_providers = [(CcInfo, "CcInfo"), (CudaInfo, "CudaInfo")],
    quality_specs = _DX_CUDA_SOURCE_SPECS,
    what = "cuda_*",
    allow_files = _CUDA_SRCS + _CUDA_HDRS,
    upstream_providers = [[CcInfo, CudaInfo]],
    extra_attrs = {
        "hdrs": attr.label_list(
            doc = "Header files exported with the CUDA library.",
            allow_files = _CUDA_HDRS,
        ),
        "check": attr.label(
            doc = "Generated check target that validates the CUDA architectures.",
            mandatory = True,
            cfg = cuda_host_transition,
        ),
        "_toolkit_check": attr.label(default = Label("//cuda/toolchain:toolkit_check"), cfg = cuda_host_transition),
    },
    runtime = "besteffort",
    extra_quality_attrs = ["hdrs"],
    upstream_cfg = cuda_host_transition,
)

# The upstream library only builds under cuda_host_transition; analyzed on its own (cquery
# over //...) it would need the dynamic CUDA runtime, so it is incompatible there.
_UPSTREAM_ONLY_UNDER_TRANSITION = select({
    Label("//cuda/toolchain:static_runtime_enabled"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

# nvcc rejects clang newer than CUDA_MAX_HOST_CLANG_MAJOR; the main @llvm is ahead of it.
_ALLOW_UNSUPPORTED_COMPILER = select({
    Label("//cc/toolchains:gcc_selected"): [],
    "//conditions:default": ["nvcc_allow_unsupported_compiler"],
})

# The host link uses the static CRT; nvcc objects default to /MD, which clashes with
# cudadevrt.lib in rdc links (LNK4098).
_STATIC_MSVCRT = select({
    Label("@platforms//os:windows"): ["static_link_msvcrt_no_debug", "static_link_msvcrt_debug"],
    "//conditions:default": [],
})

def _cuda_features(features):
    features = features or []
    if type(features) == "select":
        return features + _ALLOW_UNSUPPORTED_COMPILER + _STATIC_MSVCRT
    return list(features) + _ALLOW_UNSUPPORTED_COMPILER + _STATIC_MSVCRT

def cuda_target_compatible_with(target_compatible_with):
    """Returns the target constraints of a dx CUDA target."""
    extra = target_compatible_with if target_compatible_with != None else []
    if type(extra) == "select":
        return extra + CUDA_TARGET
    return list(extra) + CUDA_TARGET

def cuda_split_srcs(srcs):
    """Splits sources into the nvcc part and the host C/C++ part."""
    device = []
    host = []
    for src in srcs or []:
        if src.endswith(".cu") or src.endswith(".cuh"):
            device.append(src)
        else:
            host.append(src)
    return device, host

def cuda_library(name, srcs, platforms, hdrs = None, visibility = None, tags = None, target_compatible_with = None, rdc = False, **kwargs):
    """Minimal wrapper over rules_cuda cuda_library; platforms is required and rdc enables relocatable device code."""
    features = _cuda_features(kwargs.pop("features", None))
    compatible = cuda_target_compatible_with(target_compatible_with)
    _cuda_library(
        name = name + "_upstream",
        srcs = srcs,
        hdrs = hdrs or [],
        archs = platforms,
        rdc = rdc,
        tags = dx_upstream_tags(tags),
        target_compatible_with = compatible + _UPSTREAM_ONLY_UNDER_TRANSITION,
        visibility = ["//visibility:private"],
        features = features,
        **kwargs
    )
    cuda_check(
        name = name + "_cuda_check",
        archs = platforms,
        tags = ["manual"],
        target_compatible_with = compatible,
        testonly = kwargs.get("testonly", None),
        visibility = ["//visibility:private"],
    )
    forward_kwargs = {}
    for key in ("aspect_hints", "testonly"):
        if kwargs.get(key, None) != None:
            forward_kwargs[key] = kwargs[key]
    if tags != None:
        forward_kwargs["tags"] = tags
    _cuda_library_forward(
        name = name,
        upstream = name + "_upstream",
        check = name + "_cuda_check",
        srcs = srcs,
        hdrs = hdrs or [],
        target_compatible_with = compatible,
        visibility = visibility,
        **forward_kwargs
    )

def cuda_objects(name, srcs, platforms, hdrs = None, deps = None, visibility = None, tags = None, target_compatible_with = None, **kwargs):
    """Compiles .cu sources to relocatable device objects for the deps of an rdc cuda_library, cuda_binary or cuda_test."""
    features = _cuda_features(kwargs.pop("features", None))
    compatible = cuda_target_compatible_with(target_compatible_with)
    _cuda_objects(
        name = name + "_upstream",
        srcs = srcs,
        hdrs = hdrs or [],
        archs = platforms,
        deps = list(deps or []) + [Label("@rules_cuda//cuda:runtime")],
        tags = dx_upstream_tags(tags),
        target_compatible_with = compatible + _UPSTREAM_ONLY_UNDER_TRANSITION,
        visibility = ["//visibility:private"],
        features = features,
        **kwargs
    )
    cuda_check(
        name = name + "_cuda_check",
        archs = platforms,
        tags = ["manual"],
        target_compatible_with = compatible,
        testonly = kwargs.get("testonly", None),
        visibility = ["//visibility:private"],
    )
    forward_kwargs = {}
    for key in ("aspect_hints", "testonly"):
        if kwargs.get(key, None) != None:
            forward_kwargs[key] = kwargs[key]
    if tags != None:
        forward_kwargs["tags"] = tags
    _cuda_library_forward(
        name = name,
        upstream = name + "_upstream",
        check = name + "_cuda_check",
        srcs = srcs,
        hdrs = hdrs or [],
        target_compatible_with = compatible,
        visibility = visibility,
        **forward_kwargs
    )

def _cuda_executable(cc_rule, name, srcs, platforms, deps, hdrs, visibility, tags, target_compatible_with, kwargs, library_tags = None, rdc = False, cuda_copts = None):
    device, host = cuda_split_srcs(srcs)
    compatible = cuda_target_compatible_with(target_compatible_with)
    cuda_library(
        name = name + "_cuda",
        srcs = device,
        hdrs = hdrs,
        platforms = platforms,
        rdc = rdc,
        deps = deps,
        defines = kwargs.get("defines", []),
        local_defines = kwargs.get("local_defines", []),
        copts = cuda_copts or [],
        tags = dx_upstream_tags(library_tags if library_tags != None else tags),
        target_compatible_with = compatible,
        testonly = kwargs.get("testonly", None),
        visibility = ["//visibility:private"],
    )
    cc_rule(
        name = name,
        srcs = host,
        deps = (deps or []) + [name + "_cuda"],
        tags = tags,
        target_compatible_with = compatible,
        visibility = visibility,
        **kwargs
    )

def cuda_binary(name, srcs, platforms, deps = None, hdrs = None, visibility = None, tags = None, target_compatible_with = None, rdc = False, cuda_copts = None, **kwargs):
    """Compiles .cu sources with nvcc and links them into a cc_binary; cuda_copts go to nvcc."""
    _cuda_executable(_cc_binary, name, srcs, platforms, deps, hdrs, visibility, tags, target_compatible_with, kwargs, rdc = rdc, cuda_copts = cuda_copts)

_GPU_TAGS = ["exclusive", "external", "local", "requires-gpu"]

def cuda_gpu_env(min_compute_capability, min_driver):
    """Returns the env a GPU cuda_test binary reads its declared requirements from."""
    return {
        "DX_GPU_MIN_CC": min_compute_capability,
        "DX_GPU_MIN_DRIVER": min_driver,
        "DX_GPU_RUNTIME": CUDA_VERSION,
    }

def cuda_test(name, srcs, platforms, deps = None, hdrs = None, visibility = None, tags = None, target_compatible_with = None, rdc = False, min_compute_capability = None, min_driver = None, env = None, data = None, cuda_copts = None, **kwargs):
    """Compiles .cu sources into a cc_test; min_compute_capability and min_driver make it a GPU test, cuda_copts go to nvcc."""
    if (min_compute_capability == None) != (min_driver == None):
        fail("cuda_test //{}:{}: set both min_compute_capability and min_driver for a GPU test".format(native.package_name(), name))
    if min_compute_capability == None:
        if env != None:
            kwargs["env"] = env
        if data != None:
            kwargs["data"] = data
        kwargs["testonly"] = True
        _cuda_executable(_cc_test, name, srcs, platforms, deps, hdrs, visibility, tags, target_compatible_with, kwargs, rdc = rdc, cuda_copts = cuda_copts)
        return
    compatible = cuda_target_compatible_with(target_compatible_with)
    cuda_gpu_check(
        name = name + "_gpu_check",
        archs = platforms,
        min_compute_capability = min_compute_capability,
        min_driver = min_driver,
        tags = ["manual"],
        target_compatible_with = compatible,
        testonly = True,
        visibility = ["//visibility:private"],
    )
    user_tags = list(tags or [])
    kwargs["testonly"] = True
    kwargs["env"] = dict(env or {}) | cuda_gpu_env(min_compute_capability, min_driver)
    kwargs["data"] = list(data or []) + [name + "_gpu_check"]
    _cuda_executable(
        _cc_test,
        name,
        srcs,
        platforms,
        list(deps or []) + [Label("//cuda/testing:gpu_main")],
        hdrs,
        visibility,
        user_tags + [tag for tag in _GPU_TAGS if tag not in user_tags],
        target_compatible_with,
        kwargs,
        library_tags = user_tags,
        rdc = rdc,
        cuda_copts = cuda_copts,
    )
