# CUDA

dx CUDA targets compile `.cu` sources with the pinned nvcc 13.4 from the
NVIDIA redistributables. nvcc's host compiler is the main `@llvm` clang, run
with `--allow-unsupported-compiler` because the pinned nvcc lists an older
clang. Under the gcc toolchain the flag is off. The build needs no CUDA
install and no GPU.

## Setup

The CUDA rules live in the `rules_dx_cuda` module, a `bazel_dep` next to
`rules_dx`. Add this to the root `MODULE.bazel`. rules_cuda reads these tags
from the root module only, so a dependency cannot declare them for you.

```python
bazel_dep(name = "rules_dx_cuda", version = "0.0.0")
bazel_dep(name = "rules_cuda", version = "0.4.0-beta1")

cuda = use_extension("@rules_cuda//cuda:extensions.bzl", "toolchain")
cuda.redist_json(
    name = "cuda_redist",
    components = ["cccl", "crt", "cublas", "cudart", "cufft", "curand", "nvcc", "nvvm"],
    platforms = ["linux-x86_64", "linux-sbsa", "windows-x86_64", "windows-arm64"],
    sha256 = "015b97aa3977d4d0bade1631b226b6d4cbe48751cc841aad9193295df7f9047a",
    version = "13.4.2",
)
cuda.toolkit(
    name = "cuda",
    nvcc_version = "13.4",
)
use_repo(cuda, "cuda")

cuda_license = use_extension("@rules_dx_cuda//toolchain:extensions.bzl", "cuda_license")
cuda_license.license(accept = ["cuda-eula"])
```

## License

The CUDA Toolkit is licensed under the
[NVIDIA CUDA EULA](https://docs.nvidia.com/cuda/eula/index.html).
`cuda_license.license(accept = ["cuda-eula"])` accepts it for this build.
Only the root module's tag counts. Without it, every CUDA target fails
analysis. `cuda-eula` is the only license name.

Accepting the license here does not grant you one. You still need your own from NVIDIA.

## Rules

```python
load("@rules_dx_cuda//rules:defs.bzl", "cuda_binary", "cuda_library", "cuda_objects", "cuda_test")

cuda_library(
    name = "kernels",
    srcs = ["saxpy.cu"],
    hdrs = ["saxpy.cuh", "saxpy.h"],
    cuda_archs = ["compute_89:compute_89,sm_89", "sm_120"],
)
```

- `cuda_library`: compiles `srcs` with nvcc into a static library that C
  and C++ targets can depend on. `srcs` takes `.cu` files. `hdrs` takes
  `.cuh`, `.h`, `.hh`, `.hpp` and `.hxx` files. `deps`, `testonly`, `tags`
  and `visibility` work as for `cc_library`. The CUDA runtime links
  statically.
- `cuda_binary`: a `cc_binary`. Its `.cu` and `.cuh` sources go through a
  `cuda_library` with the same `cuda_archs`. The other sources compile as C++.
- `cuda_test`: the same as `cuda_binary`, as a `cc_test`.

`defines` and `local_defines` on `cuda_binary` and `cuda_test` apply to all
srcs. `copts` and `features` apply to the host srcs only. `cuda_copts` goes
to nvcc.
- `cuda_objects`: compiles `srcs` into relocatable device objects without
  linking them. It only goes into `deps` of a `cuda_library`, `cuda_binary`
  or `cuda_test` with `rdc = True`.

`cuda_library`, `cuda_binary` and `cuda_test` take `rdc` (default `False`).
`rdc = True` compiles relocatable device code and device-links it, so
`__device__` functions can call across `.cu` files and across libraries.
`rdc` supports `sm_` archs.

`cuda_archs` is required on all four. Each entry is one of:

- `sm_XY`: SASS for compute capability X.Y.
- `compute_XY`: PTX for X.Y, compiled for the device at load time.
- `compute_XY:compute_XY,sm_XY`: PTX and SASS for X.Y.

The accepted `sm_` and `compute_` values are the ones nvcc 13.4 supports:
75, 80, 86, 87, 89, 90, 100, 103, 120 and 121.

## Platforms

Linux x86_64 and Linux arm64 (sbsa) are qualified. CUDA targets are incompatible on other platforms,
so `bazel build //...` skips them.

Windows x86_64 and Windows arm64 (CUDA 13.4.2 redist) are qualified with
`--//cc:compiler=msvc`: nvcc on Windows only drives `cl.exe`. With the default
`llvm` compiler CUDA targets are incompatible there, so Windows builds skip them instead of
failing. rules_dx patches rules_cuda 0.4.0-beta1 for the windows-arm64 toolkit
(bazel-contrib/rules_cuda#528).

## Errors

| Message | Fix |
| --- | --- |
| `dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add cuda_license.license(accept = ["cuda-eula"]) to the root MODULE.bazel` | Add the license tag to the root `MODULE.bazel`. |
| `dx_cuda: unknown license '<name>'; the only one is 'cuda-eula'` | Accept `cuda-eula`. |
| `dx_cuda: cuda_archs is empty; set cuda_archs, for example cuda_archs = ["sm_89"]` | Set `cuda_archs`. |
| `dx_cuda: arch '<arch>' is not supported by nvcc 13.4; supported: ...` | Use an arch from the list. |
| `dx_cuda: @rules_cuda//cuda:compiler is '<compiler>'; dx CUDA targets compile with nvcc only` | Remove the `@rules_cuda//cuda:compiler` setting. |
| `dx_cuda: the CUDA toolkit is not the pinned redistributable (nvcc at '<path>'); declare the cuda.redist_json and cuda.toolkit tags from the dx CUDA README` | Add the `cuda.redist_json` and `cuda.toolkit` tags from Setup. |
| `dx_cuda: the CUDA toolkit is <version>; dx CUDA targets need 13.4` | Use the versions from Setup. |
| `dx_cuda: no nvcc toolchain for this target; dx CUDA targets build for linux x86_64` | Build for Linux x86_64. |
| nvcc fails with `unsupported clang version!` | Move the stanza before `register_toolchains("@llvm//toolchain:all")`. |

## Library components

The pinned stanza declares cuBLAS 13.8.0.4, cuFFT 12.4.0.43 and cuRAND
10.4.4.72 next to the compiler components, from the CUDA 13.4.3
redistributable manifest. Bazel fetches an archive only when a target uses it
(cuBLAS 877 MB, cuFFT 249 MB, cuRAND 87 MB).

Depend on the license-gated labels:

```python
cuda_library(
    name = "gemm",
    srcs = ["gemm.cu"],
    cuda_archs = ["sm_89"],
    deps = ["@rules_dx_cuda//components:cublas"],  # or :cufft, :curand
)
```

These targets fail unless the NVIDIA EULA is accepted and the files come from
the pinned redistributable. Never depend on `@cuda//:cublas`, which skips both
checks, or on `@cuda//:cuda_headers` or `@cuda//:cuda_all_files`, which fetch
every component. The `cufft` and `curand` targets are tagged `manual`.

If your stanza has no `cuda_cublas` tag, building these targets fails with
`no such target @cuda//:cublas`; add the `cuda_cublas` tag to your `cuda` stanza when the tree has `.cu` files. See
`tests/fixtures/cublas`.

## GPU tests

```python
load("@rules_dx_cuda//rules:defs.bzl", "cuda_test")

cuda_test(
    name = "saxpy_gpu_test",
    srcs = ["saxpy_gpu_test.cu"],
    cuda_archs = ["compute_89:compute_89,sm_89", "sm_120"],
    min_compute_capability = "8.9",
    min_driver = "13.0",
    tags = ["manual"],
    deps = [":saxpy"],
)
```

```sh
bazel test //:saxpy_gpu_test --test_output=streamed
```

The test source implements `int dx_gpu_test_main(void)` from
`testing/gpu_test.h`. Run the kernels there, compute a CPU
reference, and record each comparison with
`dx_gpu_check(name, want, got, n, abs_tol, rel_tol)`. Pass `got = NULL`
when a kernel launch fails. Return 0.

Set `min_compute_capability` and `min_driver` on `cuda_test` to run it on a GPU. Set both.

- `cuda_archs`: required arch specs, as for `cuda_library`.
- `min_compute_capability`: lowest device compute capability, `X.Y`.
  It must be at least the lowest arch in `cuda_archs`.
- `min_driver`: lowest CUDA driver version, `X.Y`, from `13.0` to `13.4`.

The runtime version is always the pinned toolkit, 13.4.

Every GPU `cuda_test` gets the tags `requires-gpu`, `external`, `local`
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
