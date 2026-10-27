"""Minimal CUDA wrappers that compile with the pinned nvcc."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_cuda//cuda:defs.bzl", _cuda_library = "cuda_library")
load("@rules_cuda//cuda/private:providers.bzl", "CudaInfo")
load("//cc/rules:defs.bzl", _cc_binary = "cc_binary", _cc_test = "cc_test")
load("//cuda/rules:components.bzl", _cuda_component_library = "cuda_component_library")
load("//cuda/toolchain:check.bzl", "cuda_check", "cuda_gpu_check")
load("//cuda/toolchain:transition.bzl", "cuda_host_transition")
load("//libs/starlark:wrapper.bzl", "dx_library_forward_rule", "dx_upstream_tags")
load("//modules:cuda.bzl", "CUDA_VERSION")
load("//quality:sources.bzl", "QualitySourcesInfo")

cuda_component_library = _cuda_component_library

_CUDA_TARGET_NAMES = [
    "@platforms//os:linux",
    "@platforms//cpu:x86_64",
]

# Labels, so a consumer without its own platforms dependency can use them.
CUDA_TARGET = [Label(name) for name in _CUDA_TARGET_NAMES]

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
        "hdrs": attr.label_list(allow_files = _CUDA_HDRS),
        "check": attr.label(mandatory = True, cfg = cuda_host_transition),
        "_toolkit_check": attr.label(default = Label("//cuda/toolchain:toolkit_check"), cfg = cuda_host_transition),
    },
    runtime = "besteffort",
    extra_quality_attrs = ["hdrs"],
    upstream_cfg = cuda_host_transition,
)

# The upstream library only builds under cuda_host_transition; analyzed on its own (cquery
# over //...) it would need the dynamic CUDA runtime, so it is incompatible there.
_UPSTREAM_ONLY_UNDER_TRANSITION = select({
    Label("//cuda/toolchain:host_compiler_enabled"): [],
    "//conditions:default": [Label("@platforms//:incompatible")],
})

def cuda_target_compatible_with(target_compatible_with):
    """Returns the target constraints of a dx CUDA target."""
    extra = target_compatible_with if target_compatible_with != None else []
    return CUDA_TARGET + [c for c in extra if c not in CUDA_TARGET and c not in _CUDA_TARGET_NAMES]

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

def cuda_library(name, srcs, archs, hdrs = None, visibility = None, tags = None, target_compatible_with = None, **kwargs):
    """Minimal wrapper over rules_cuda cuda_library; archs is required."""
    compatible = cuda_target_compatible_with(target_compatible_with)
    _cuda_library(
        name = name + "_upstream",
        srcs = srcs,
        hdrs = hdrs or [],
        archs = archs,
        tags = dx_upstream_tags(tags),
        target_compatible_with = compatible + _UPSTREAM_ONLY_UNDER_TRANSITION,
        visibility = ["//visibility:private"],
        **kwargs
    )
    cuda_check(
        name = name + "_cuda_check",
        archs = archs,
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

def _cuda_executable(cc_rule, name, srcs, archs, deps, hdrs, visibility, tags, target_compatible_with, kwargs, library_tags = None):
    device, host = cuda_split_srcs(srcs)
    compatible = cuda_target_compatible_with(target_compatible_with)
    cuda_library(
        name = name + "_cuda",
        srcs = device,
        hdrs = hdrs,
        archs = archs,
        deps = deps,
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

def cuda_binary(name, srcs, archs, deps = None, hdrs = None, visibility = None, tags = None, target_compatible_with = None, **kwargs):
    """Compiles .cu sources with nvcc and links them into a cc_binary."""
    _cuda_executable(_cc_binary, name, srcs, archs, deps, hdrs, visibility, tags, target_compatible_with, kwargs)

def cuda_test(name, srcs, archs, deps = None, hdrs = None, visibility = None, tags = None, target_compatible_with = None, **kwargs):
    """Compiles .cu sources with nvcc and links them into a cc_test."""
    kwargs["testonly"] = True
    _cuda_executable(_cc_test, name, srcs, archs, deps, hdrs, visibility, tags, target_compatible_with, kwargs)

_GPU_TAGS = ["exclusive", "external", "local", "requires-gpu"]

def cuda_gpu_env(min_compute_capability, min_driver):
    """Returns the env a cuda_gpu_test binary reads its declared requirements from."""
    return {
        "DX_GPU_MIN_CC": min_compute_capability,
        "DX_GPU_MIN_DRIVER": min_driver,
        "DX_GPU_RUNTIME": CUDA_VERSION,
    }

def cuda_gpu_test(name, srcs, archs, min_compute_capability, min_driver, deps = None, hdrs = None, visibility = None, tags = None, env = None, data = None, target_compatible_with = None, **kwargs):
    """Runs dx_gpu_test_main on a real GPU and records gpu_facts.json; a host without a GPU fails."""
    compatible = cuda_target_compatible_with(target_compatible_with)
    cuda_gpu_check(
        name = name + "_gpu_check",
        archs = archs,
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
        archs,
        list(deps or []) + [Label("//cuda/testing:gpu_main")],
        hdrs,
        visibility,
        user_tags + [tag for tag in _GPU_TAGS if tag not in user_tags],
        target_compatible_with,
        kwargs,
        library_tags = user_tags,
    )
