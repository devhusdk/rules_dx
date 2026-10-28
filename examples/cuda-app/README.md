# CUDA kernels example

A multi-package CUDA app. A `cuda_library` holds three kernels behind a C ABI header, and a C++ `cuda_app` binary calls them. Each kernel has a CPU version, so the app gives the same output on a host without a GPU.

| Package | Role |
| --- | --- |
| `kernels` | Vector add, block reduction and 1D stencil kernels, shared `common.cuh`, C ABI `kernels.h` |
| `app` | `cuda_app` binary and its output test |
| `gpu` | `gpu_test`, a `cuda_gpu_test` that checks the three kernels against their CPU versions on a real GPU |
| `checks` | Device code and format checks |

```sh
bazel build //examples/cuda-app/...
bazel test //examples/cuda-app/...
bazel run //examples/cuda-app/app:cuda_app
```

`cuda_app` adds two vectors, runs the stencil over the sums and reduces both arrays. It prints one line:

```text
cuda-app n=4096 add=25159680 stencil=75466755
```

It uses the GPU when the host has a CUDA device and the kernels run. Otherwise it runs the CPU versions. Both print the same line. The exit code is always `0`, and `output_test` checks the line.

The kernels build for `compute_80:compute_80,sm_80` and `sm_90`. That is PTX and SASS for compute capability 8.0 and SASS for 9.0. `device_code_test` reads the fatbins in the static archive and the linked binary and expects those three entries. It also checks that the binary links the CUDA runtime statically. `format_test` checks every source against the repository `.clang-format`.

nvcc and its host compiler come from the pinned CUDA redistributables and LLVM toolchain declared in `MODULE.bazel`, so the build needs no CUDA install and no GPU. The targets build on Linux x86_64 only. Other platforms skip them as incompatible.

`gpu_test` is tagged `manual`, so `//...` skips it. It fails on a host without a GPU. On a GPU host run it with `bazel test //examples/cuda-app/gpu:gpu_test`, or through a nested consumer with `bazel test //tools/testing/hermetic:cuda_gpu_nested_test`. The kernels carry PTX for compute capability 8.0, so newer GPUs such as an RTX 50 series card JIT them.
