"""Configuration for the nvcc compile of dx CUDA libraries."""

_STATIC_RUNTIME = str(Label("//cuda/private/toolchain:static_runtime"))
_RUNTIME = "@rules_cuda//cuda:runtime"

def _impl(_settings, _attr):
    return {
        _STATIC_RUNTIME: True,
        _RUNTIME: Label("//cuda/private/toolchain:runtime"),
    }

cuda_host_transition = transition(
    implementation = _impl,
    inputs = [],
    outputs = [_STATIC_RUNTIME, _RUNTIME],
)
