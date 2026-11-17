#ifndef DX_CUDA_APP_COMMON_CUH_
#define DX_CUDA_APP_COMMON_CUH_

#include <cuda_runtime.h>

#include <cstddef>

constexpr int kDxBlock = 256;

inline int dx_grid(int n) { return (n + kDxBlock - 1) / kDxBlock; }

inline size_t dx_bytes(int n) { return static_cast<size_t>(n) * sizeof(int); }

__host__ __device__ inline int dx_add(int a, int b) { return a + b; }

__host__ __device__ inline int dx_stencil_at(int n, const int *in, int i) {
  int left = i > 0 ? in[i - 1] : 0;
  int right = i + 1 < n ? in[i + 1] : 0;
  return left + in[i] + right;
}

inline int *dx_device_copy(int n, const int *host) {
  int *device = nullptr;
  if (cudaMalloc(&device, dx_bytes(n)) != cudaSuccess) {
    return nullptr;
  }
  if (host != nullptr && cudaMemcpy(device, host, dx_bytes(n),
                                    cudaMemcpyHostToDevice) != cudaSuccess) {
    cudaFree(device);
    return nullptr;
  }
  return device;
}

#endif
