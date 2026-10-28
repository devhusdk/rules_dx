# CUDA

dx CUDA targets compile `.cu` sources with the pinned nvcc 13.4 from the
NVIDIA redistributables. nvcc's host compiler is a pinned LLVM 22 with its
own sysroot. The build needs no CUDA install and no GPU.

## Setup

Add this to the root `MODULE.bazel`. rules_cuda and toolchains_llvm read
these tags from the root module only, so a dependency cannot declare them
for you.

Put the stanza before `register_toolchains("@llvm//toolchain:all")`.
Toolchains resolve in registration order, and `@cuda_host_llvm` applies only
when `@@rules_dx+//cuda/toolchain:host_compiler_enabled` is set. If
`@llvm//toolchain:all` comes first, nvcc gets a host compiler it does not
support.

```python
bazel_dep(name = "rules_cuda", version = "0.3.0")
bazel_dep(name = "toolchains_llvm", version = "1.12.0")

cuda = use_extension("@rules_cuda//cuda:extensions.bzl", "toolchain")
cuda.component(
    name = "cuda_cccl",
    component_name = "cccl",
    sha256 = "6b7516074f42f80dd3feb9c11318c8e1c5ab56cdd9f18fff4c5ace2e7e0a8209",
    strip_prefix = "cccl-linux-x86_64-13.3.4.3.1-archive",
    url = "https://developer.download.nvidia.com/compute/cuda/redist/cccl/linux-x86_64/cccl-linux-x86_64-13.3.4.3.1-archive.tar.xz",
    version = "13.3.4.3.1",
)
cuda.component(
    name = "cuda_crt",
    component_name = "crt",
    sha256 = "c969e61ded12dbf0cf20ace72867f1622ec0e8ea1e1d305e27761012025f7aaa",
    strip_prefix = "cuda_crt-linux-x86_64-13.4.92-archive",
    url = "https://developer.download.nvidia.com/compute/cuda/redist/cuda_crt/linux-x86_64/cuda_crt-linux-x86_64-13.4.92-archive.tar.xz",
    version = "13.4.92",
)
cuda.component(
    name = "cuda_cudart",
    component_name = "cudart",
    sha256 = "0ac5dbc538d04e9983bc493b410cce4b459e1ee9f5f6654b6464ef7b3e14a8b5",
    strip_prefix = "cuda_cudart-linux-x86_64-13.4.92-archive",
    url = "https://developer.download.nvidia.com/compute/cuda/redist/cuda_cudart/linux-x86_64/cuda_cudart-linux-x86_64-13.4.92-archive.tar.xz",
    version = "13.4.92",
)
cuda.component(
    name = "cuda_nvcc",
    component_name = "nvcc",
    sha256 = "60998f40cc9df5826b2a846e2dcc328133fbc5ef539398ce0a21cf5dcac7193d",
    strip_prefix = "cuda_nvcc-linux-x86_64-13.4.92-archive",
    url = "https://developer.download.nvidia.com/compute/cuda/redist/cuda_nvcc/linux-x86_64/cuda_nvcc-linux-x86_64-13.4.92-archive.tar.xz",
    version = "13.4.92",
)
cuda.component(
    name = "cuda_nvvm",
    component_name = "nvvm",
    sha256 = "0e619cf3d5b27f7c812665599ca9761fc296e56129b24a85d822a5d8a1a3bd75",
    strip_prefix = "libnvvm-linux-x86_64-13.4.92-archive",
    url = "https://developer.download.nvidia.com/compute/cuda/redist/libnvvm/linux-x86_64/libnvvm-linux-x86_64-13.4.92-archive.tar.xz",
    version = "13.4.92",
)
cuda.toolkit(
    name = "cuda",
    components_mapping = {
        "cccl": "@cuda_cccl",
        "crt": "@cuda_crt",
        "cudart": "@cuda_cudart",
        "nvcc": "@cuda_nvcc",
        "nvvm": "@cuda_nvvm",
    },
    nvcc_version = "13.4",
    version = "13.4",
)
use_repo(cuda, "cuda")

cuda_host_sysroot = use_repo_rule("@toolchains_llvm//toolchain:sysroot.bzl", "sysroot")

cuda_host_sysroot(
    name = "cuda_host_sysroot",
    sha256 = "84656a6df544ecef62169cfe3ab6e41bb4346a62d3ba2a045dc5a0a2ecea94a3",
    urls = ["https://commondatastorage.googleapis.com/chrome-linux-sysroot/toolchain/2202c161310ffde63729f29d27fe7bb24a0bc540/debian_stretch_amd64_sysroot.tar.xz"],
)

cuda_host_llvm = use_extension("@toolchains_llvm//toolchain/extensions:llvm.bzl", "llvm")
cuda_host_llvm.toolchain(
    name = "cuda_host_llvm",
    llvm_version = "22.1.8",
    target_settings = {"": ["@@rules_dx+//cuda/toolchain:host_compiler_enabled"]},
)
cuda_host_llvm.sysroot(
    name = "cuda_host_llvm",
    label = "@cuda_host_sysroot//sysroot",
    targets = ["linux-x86_64"],
)
use_repo(cuda_host_llvm, "cuda_host_llvm")

register_toolchains("@cuda_host_llvm//:all")

cuda_license = use_extension("@rules_dx//cuda/toolchain:repos.bzl", "cuda_license")
cuda_license.license(accept = ["cuda-eula"])
```

`target_settings` takes the canonical label `@@rules_dx+//cuda/toolchain:host_compiler_enabled`.
toolchains_llvm writes it as text into a generated repository, where the
apparent name `@rules_dx` does not resolve.

## License

The CUDA Toolkit is licensed under the
[NVIDIA CUDA EULA](https://docs.nvidia.com/cuda/eula/index.html).
`cuda_license.license(accept = ["cuda-eula"])` accepts it for this build.
Only the root module's tag counts. Without it, every CUDA target fails
analysis. `cuda-eula` is the only license name.

## Rules

```python
load("@rules_dx//cuda/rules:defs.bzl", "cuda_binary", "cuda_library", "cuda_objects", "cuda_test")

cuda_library(
    name = "kernels",
    srcs = ["saxpy.cu"],
    hdrs = ["saxpy.cuh", "saxpy.h"],
    archs = ["compute_89:compute_89,sm_89", "sm_120"],
)
```

- `cuda_library`: compiles `srcs` with nvcc into a static library that C
  and C++ targets can depend on. `srcs` takes `.cu` files. `hdrs` takes
  `.cuh`, `.h`, `.hh`, `.hpp` and `.hxx` files. `deps`, `testonly`, `tags`
  and `visibility` work as for `cc_library`. The CUDA runtime links
  statically.
- `cuda_binary`: a `cc_binary`. Its `.cu` and `.cuh` sources go through a
  `cuda_library` with the same `archs`. The other sources compile as C++.
- `cuda_test`: the same as `cuda_binary`, as a `cc_test`.
- `cuda_objects`: compiles `srcs` into relocatable device objects without
  linking them. It only goes into `deps` of a `cuda_library`, `cuda_binary`
  or `cuda_test` with `rdc = True`.

`cuda_library`, `cuda_binary` and `cuda_test` take `rdc` (default `False`).
`rdc = True` compiles relocatable device code and device-links it, so
`__device__` functions can call across `.cu` files and across libraries.
`rdc` supports `sm_` archs.

`archs` is required on all four. Each entry is one of:

- `sm_XY`: SASS for compute capability X.Y.
- `compute_XY`: PTX for X.Y, compiled for the device at load time.
- `compute_XY:compute_XY,sm_XY`: PTX and SASS for X.Y.

The accepted `sm_` and `compute_` values are the ones nvcc 13.4 supports:
75, 80, 86, 87, 89, 90, 100, 103, 120 and 121.

## Platforms

CUDA targets build for Linux x86_64 only. On other platforms they are
incompatible, so `bazel build //...` skips them.

## Errors

| Message | Fix |
| --- | --- |
| `dx_cuda: the CUDA Toolkit EULA (https://docs.nvidia.com/cuda/eula/index.html) is not accepted; add cuda_license.license(accept = ["cuda-eula"]) to the root MODULE.bazel` | Add the license tag to the root `MODULE.bazel`. |
| `dx_cuda: unknown license '<name>'; the only one is 'cuda-eula'` | Accept `cuda-eula`. |
| `dx_cuda: archs is empty; set archs, for example archs = ["sm_89"]` | Set `archs`. |
| `dx_cuda: arch '<arch>' is not supported by nvcc 13.4; supported: ...` | Use an arch from the list. |
| `dx_cuda: @rules_cuda//cuda:compiler is '<compiler>'; dx CUDA targets compile with nvcc only` | Remove the `@rules_cuda//cuda:compiler` setting. |
| `dx_cuda: the CUDA toolkit is not the pinned redistributable (nvcc at '<path>'); declare the cuda.component and cuda.toolkit tags from the dx CUDA README` | Add the `cuda.component` and `cuda.toolkit` tags from Setup. |
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
    archs = ["sm_89"],
    deps = ["@rules_dx//cuda/components:cublas"],  # or :cufft, :curand
)
```

These targets fail unless the NVIDIA EULA is accepted and the files come from
the pinned redistributable. Never depend on `@cuda//:cublas`, which skips both
checks, or on `@cuda//:cuda_headers` or `@cuda//:cuda_all_files`, which fetch
every component. The `cufft` and `curand` targets are tagged `manual`.

If your stanza has no `cuda_cublas` tag, building these targets fails with
`no such target @cuda//:cublas`; copy the stanza that `dx init` writes to
`MODULE.bazel.snippet` when the tree has `.cu` files. See
`cuda/tests/fixtures/cublas`.

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
