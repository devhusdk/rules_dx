# CUDA

## GPU tests

```python
load("@rules_dx//cuda/rules:defs.bzl", "cuda_gpu_test")

cuda_gpu_test(
    name = "saxpy_gpu_test",
    srcs = ["saxpy_gpu_test.cu"],
    archs = ["compute_89:compute_89,sm_89", "sm_120"],
    min_compute_capability = "8.9",
    min_driver = "13.0",
    tags = ["manual"],
    deps = [":saxpy"],
)
```

```sh
bazel test //cuda/tests:saxpy_gpu_test --test_output=streamed
```

The test source implements `int dx_gpu_test_main(void)` from
`cuda/testing/gpu_test.h`. Run the kernels there, compute a CPU
reference, and record each comparison with
`dx_gpu_check(name, want, got, n, abs_tol, rel_tol)`. Pass `got = NULL`
when a kernel launch fails. Return 0.

Attributes:

- `archs`: required arch specs, as for `cuda_library`.
- `min_compute_capability`: lowest device compute capability, `X.Y`.
  It must be at least the lowest arch in `archs`.
- `min_driver`: lowest CUDA driver version, `X.Y`, from `13.0` to `13.4`.
- `srcs`, `hdrs`, `deps`, `env`, `data`, `tags`: as for `cuda_test`.

The runtime version is always the pinned toolkit, 13.4.

Every `cuda_gpu_test` gets the tags `requires-gpu`, `external`, `local`
and `exclusive`. It never adds `manual`.

Each run prints one `dx-gpu-facts: {...}` line and writes
`gpu_facts.json` to `bazel-testlogs/<package>/<name>/test.outputs/`
(as `outputs.zip` when Bazel zips undeclared outputs; pass
`--nozip_undeclared_test_outputs` for the plain file).
The file has the schema `dx.cuda.gpu_facts.v1` and records the status,
the reason, the declared versions, the driver, the runtime, the device
name, compute capability and memory, and every check.

| Status | Meaning | Exit code |
| --- | --- | --- |
| `pass` | Every recorded check is within tolerance. | 0 |
| `fail` | A check failed, no check ran, or a declared value is missing. | 1 |
| `unavailable` | No GPU, no driver, or a CUDA call failed. | 1 |
| `unsupported` | The device or driver is below the minimum, or the runtime differs. | 1 |

When the status is not `pass`, the first stderr line is
`<status>: <reason>`. A host without a GPU fails with `unavailable:`.
It is never skipped.

On CI lanes without a GPU, exclude the tests:

```sh
bazel test //... --test_tag_filters=-requires-gpu
```
