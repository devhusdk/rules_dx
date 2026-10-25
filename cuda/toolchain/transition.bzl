"""Configuration for the nvcc compile of dx CUDA libraries."""

_HOST_COMPILER = str(Label("//cuda/toolchain:host_compiler"))
_RUNTIME = "@rules_cuda//cuda:runtime"

def _impl(_settings, _attr):
    return {
        _HOST_COMPILER: True,
        _RUNTIME: Label("@cuda//:cuda_runtime_static"),
    }

cuda_host_transition = transition(
    implementation = _impl,
    inputs = [],
    outputs = [_HOST_COMPILER, _RUNTIME],
)
