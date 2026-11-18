"""Configuration for the nvcc compile of dx CUDA libraries."""

load("//cc/private:defs.bzl", "CC_COVERAGE_INPUTS", "cc_coverage_settings")

_STATIC_RUNTIME = str(Label("//cuda/private/toolchain:static_runtime"))
_RUNTIME = "@rules_cuda//cuda:runtime"
_COPT = "//command_line_option:copt"
CUDA_HOST_COPTS_SETTING = "@rules_cuda//cuda:host_copts"
CUDA_HOST_COMPILER_SETTING = str(Label("//cc:compiler"))
CUDA_HOST_INPUTS = CC_COVERAGE_INPUTS + [_COPT, CUDA_HOST_COPTS_SETTING]

def _append(value, extra):
    current = [value] if type(value) == "string" else list(value)
    return [v for v in current if v] + [v for v in extra if v not in current]

def cuda_host_settings(settings, host_compiler, host_copts):
    """Returns the nvcc compile settings: the static runtime, the host compiler and its copts."""
    out = {
        _STATIC_RUNTIME: True,
        _RUNTIME: Label("//cuda/private/toolchain:runtime"),
        CUDA_HOST_COMPILER_SETTING: host_compiler or settings[CUDA_HOST_COMPILER_SETTING],
        _COPT: settings[_COPT],
        CUDA_HOST_COPTS_SETTING: settings[CUDA_HOST_COPTS_SETTING],
    }
    if host_copts:
        out[_COPT] = _append(settings[_COPT], host_copts)
        out[CUDA_HOST_COPTS_SETTING] = _append(settings[CUDA_HOST_COPTS_SETTING], host_copts)
    out.update(cc_coverage_settings(settings))
    return out

def _impl(settings, attr):
    return cuda_host_settings(settings, attr.host_compiler, attr.host_copts)

cuda_host_transition = transition(
    implementation = _impl,
    inputs = CUDA_HOST_INPUTS,
    outputs = [_STATIC_RUNTIME, _RUNTIME, CUDA_HOST_COMPILER_SETTING, _COPT, CUDA_HOST_COPTS_SETTING, "//command_line_option:features"],
)
