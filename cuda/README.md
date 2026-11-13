# CUDA

dx CUDA targets compile `.cu` sources with the pinned nvcc 13.4 from the
NVIDIA redistributables. nvcc's host compiler is the main `@llvm` clang, run
with `--allow-unsupported-compiler` because the pinned nvcc lists an older
clang. Under the gcc toolchain the flag is off. The build needs no CUDA
install and no GPU.

## Dependencies

### Library components

The pinned stanza declares cuBLAS 13.8.0.4, cuFFT 12.4.0.43 and cuRAND
10.4.4.72 next to the compiler components, from the CUDA 13.4.3
redistributable manifest. Bazel fetches an archive only when a target uses it
(cuBLAS 877 MB, cuFFT 249 MB, cuRAND 87 MB).

Depend on the pinned labels:

```python
cuda_library(
    name = "gemm",
    srcs = ["gemm.cu"],
    cuda_archs = ["sm_89"],
    deps = ["@rules_dx//cuda:cublas"],  # or :cufft, :curand
)
```

These targets fail unless the files come from the pinned redistributable. Never depend on `@cuda//:cublas`, which skips the
check, or on `@cuda//:cuda_headers` or `@cuda//:cuda_all_files`, which fetch
every component. The `cufft` and `curand` targets are tagged `manual`.

See `cuda/tests/fixtures/cublas`.

## Quality

### GPU tests

```python
load("@rules_dx//cuda:defs.bzl", "cuda_test")

cuda_test(
    name = "saxpy_gpu_test",
    srcs = ["saxpy_gpu_test.cu"],
    cuda_archs = ["compute_89:compute_89,sm_89", "sm_120"],
    min_compute_capability = "8.9",
    min_driver = "13.0",
    deps = [":saxpy"],
)
```

A machine with a GPU declares it with three flags, for example in its `user.bazelrc`:

```text
common --@rules_dx//cuda:gpu_compute_capability=12.0
common --@rules_dx//cuda:gpu_driver=13.4
common --local_resources=gpu=1
```

- `gpu_compute_capability`: compute capability of the device, `X.Y`.
- `gpu_driver`: CUDA driver version of the machine, `X.Y`.
- `--local_resources=gpu=<n>`: number of GPU tests that run at once.

Set both `gpu_` flags or neither. Without them every GPU `cuda_test` is incompatible, so `bazel test //...` skips it.

```sh
bazel test //:saxpy_gpu_test --test_output=streamed
```

The test source implements `int dx_gpu_test_main(void)` from
`dx/gpu_check.h`. Run the kernels there, compute a CPU
reference, and record each comparison with
`dx_gpu_check(name, want, got, n, abs_tol, rel_tol)`. Pass `got = NULL`
when a kernel launch fails. Return 0.

Set `min_compute_capability` and `min_driver` on `cuda_test` to run it on a GPU. Set both.

- `cuda_archs`: required arch specs, as for `cuda_library`.
- `min_compute_capability`: lowest device compute capability, `X.Y`.
  It must be at least the lowest arch in `cuda_archs`.
- `min_driver`: lowest CUDA driver version, `X.Y`, from `13.0` to `13.4`.

The runtime version is always the pinned toolkit, 13.4.

Every GPU `cuda_test` gets the tags `requires-gpu` and `resources:gpu:1`.
It runs sandboxed and its result is cached. The declared device, the
minimums and the runtime version are inputs of the test.

Each run prints one `dx-gpu-facts: {...}` line and writes
`gpu_facts.json` to `bazel-testlogs/<package>/<name>/test.outputs/`
(as `outputs.zip` when Bazel zips undeclared outputs; pass
`--nozip_undeclared_test_outputs` for the plain file).
The file has the schema `dx.cuda.gpu_facts.v1` and records the status,
the reason, the declared versions and device, the driver, the runtime, the device
name, compute capability and memory, and every check.

| Status | Meaning | Exit code |
| --- | --- | --- |
| `pass` | Every recorded check is within tolerance. | 0 |
| `fail` | A check failed, no check ran, a declared value is missing, or the device or driver is not the declared one. | 1 |
| `unavailable` | A device is declared, but there is no GPU, no driver, or a CUDA call failed. | 1 |
| `unsupported` | The device or driver is below the minimum, or the runtime differs. | 1 |

When the status is not `pass`, the first stderr line is
`<status>: <reason>`.

## Toolchain

### Setup

The CUDA rules load from `@rules_dx//cuda:defs.bzl`. rules_dx declares the
pinned toolkit and the nvcc toolchain. Add this to the root `MODULE.bazel`:

```python
cuda = use_extension("@rules_dx//cuda:extensions.bzl", "cuda")
dx_licenses = use_extension("@rules_dx//core:extensions.bzl", "dx_licenses")
dx_licenses.accept(ids = ["cuda-eula"])
```

### Toolchain

rules_dx declares the pinned nvcc toolchain; a consumer adds no
`register_toolchains` line. The one tag a consumer adds is
`dx_licenses.accept(ids = ["cuda-eula"])`.

### Errors

| Message | Fix |
| --- | --- |
| `dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add dx_licenses.accept(ids = ["cuda-eula"]) to the root MODULE.bazel` | Add the license tag to the root `MODULE.bazel`. |
| `dx_cuda: unknown license '<name>'; the only one is 'cuda-eula'` | Accept `cuda-eula`. |
| `dx_cuda: cuda_archs is empty; set cuda_archs, for example cuda_archs = ["sm_89"]` | Set `cuda_archs`. |
| `dx_cuda: arch '<arch>' is not supported by nvcc 13.4; supported: ...` | Use an arch from the list. |
| `dx_cuda: @rules_cuda//cuda:compiler is '<compiler>'; dx CUDA targets compile with nvcc only` | Remove the `@rules_cuda//cuda:compiler` setting. |
| `dx_cuda: the CUDA toolkit is not the pinned redistributable (nvcc at '<path>'); use the cuda extension from @rules_dx//cuda:extensions.bzl and remove your own cuda.redist_json and cuda.toolkit tags` | Remove your own `cuda.redist_json` and `cuda.toolkit` tags and use the `cuda` extension from `@rules_dx//cuda:extensions.bzl`. |
| `dx_cuda: the CUDA toolkit is <version>; dx CUDA targets need 13.4` | Use the versions from Setup. |
| `dx_cuda: no nvcc toolchain for this target; dx CUDA targets build for linux_x86_64, linux_arm64, windows_x86_64, windows_arm64` | Build for one of the listed platforms. |
| nvcc fails with `unsupported clang version!` | Move the stanza before `register_toolchains("@llvm//toolchain:all")`. |

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).

Windows needs `--//cc:compiler=msvc`: nvcc on Windows only drives `cl.exe`. With the default `llvm` compiler CUDA targets are incompatible there and Windows builds skip them.

## License

The CUDA Toolkit is licensed under the
[NVIDIA CUDA EULA](https://docs.nvidia.com/cuda/eula/index.html).
`dx_licenses.accept(ids = ["cuda-eula"])` accepts it for this build.
Only the root module's tag counts. Without it, fetching the CUDA redistributables fails. `cuda-eula` is the only license name.

Accepting the license here does not grant you one. You still need your own from NVIDIA.
