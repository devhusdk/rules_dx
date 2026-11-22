"""Configuration for the nvcc compile of dx CUDA libraries and the link of dx CUDA executables."""

load("//cc/private:defs.bzl", "CC_COVERAGE_INPUTS", "cc_coverage_settings")

_STATIC_RUNTIME = str(Label("//cuda/private/toolchain:static_runtime"))
_RUNTIME = "@rules_cuda//cuda:runtime"
CUDA_HOST_COMPILER_SETTING = str(Label("//cc:compiler"))

def cuda_host_settings(settings, host_compiler):
    """Returns the nvcc compile and link settings: the static runtime and the host compiler."""
    out = {
        _STATIC_RUNTIME: True,
        _RUNTIME: Label("//cuda/private/toolchain:runtime"),
        CUDA_HOST_COMPILER_SETTING: host_compiler or settings[CUDA_HOST_COMPILER_SETTING],
    }
    out.update(cc_coverage_settings(settings))
    return out

def _impl(settings, attr):
    return cuda_host_settings(settings, attr.host_compiler)

cuda_host_transition = transition(
    implementation = _impl,
    inputs = CC_COVERAGE_INPUTS,
    outputs = [_STATIC_RUNTIME, _RUNTIME, CUDA_HOST_COMPILER_SETTING, "//command_line_option:features"],
)
