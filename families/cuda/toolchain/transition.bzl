"""Configuration for the nvcc compile of dx CUDA libraries."""

load("@rules_dx//cc/rules:defs.bzl", "CC_COVERAGE_INPUTS", "cc_coverage_settings")

_STATIC_RUNTIME = str(Label("//toolchain:static_runtime"))
_RUNTIME = "@rules_cuda//cuda:runtime"

def cuda_host_settings(settings):
    """Returns the nvcc compile settings: the static runtime, and LLVM coverage maps for the host code under coverage."""
    out = {
        _STATIC_RUNTIME: True,
        _RUNTIME: Label("//toolchain:runtime"),
    }
    out.update(cc_coverage_settings(settings))
    return out

def _impl(settings, _attr):
    return cuda_host_settings(settings)

cuda_host_transition = transition(
    implementation = _impl,
    inputs = CC_COVERAGE_INPUTS,
    outputs = [_STATIC_RUNTIME, _RUNTIME, "//command_line_option:features"],
)
