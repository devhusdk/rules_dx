#include <cublas_v2.h>
#include <cuda_runtime.h>

#include "cuda/tests/fixtures/cublas/cublas_saxpy.h"

namespace {

__global__ void dx_cublas_kernel(int n, float a, const float* x, float* y) {
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i < n) {
    y[i] = a * x[i] + y[i];
  }
}

// Copies x and y to the device, runs `compute` on them and copies y back.
template <typename Compute>
int RunOnDevice(int n, const float* x, float* y, Compute compute) {
  size_t bytes = static_cast<size_t>(n) * sizeof(float);
  float* dx = nullptr;
  float* dy = nullptr;
  if (cudaMalloc(&dx, bytes) != cudaSuccess) {
    return 1;
  }
  if (cudaMalloc(&dy, bytes) != cudaSuccess) {
    cudaFree(dx);
    return 1;
  }
  cudaMemcpy(dx, x, bytes, cudaMemcpyHostToDevice);
  cudaMemcpy(dy, y, bytes, cudaMemcpyHostToDevice);
  int failed = compute(dx, dy);
  if (cudaDeviceSynchronize() != cudaSuccess) {
    failed = 1;
  }
  cudaMemcpy(y, dy, bytes, cudaMemcpyDeviceToHost);
  cudaFree(dx);
  cudaFree(dy);
  return failed;
}

}  // namespace

extern "C" int dx_cublas_device_count(void) {
  int count = 0;
  if (cudaGetDeviceCount(&count) != cudaSuccess) {
    return 0;
  }
  return count;
}

extern "C" int dx_cublas_saxpy_gpu(int n, float a, const float* x, float* y) {
  return RunOnDevice(n, x, y, [&](float* dx, float* dy) {
    cublasHandle_t handle = nullptr;
    if (cublasCreate(&handle) != CUBLAS_STATUS_SUCCESS) {
      return 1;
    }
    int failed = cublasSaxpy(handle, n, &a, dx, 1, dy, 1) != CUBLAS_STATUS_SUCCESS;
    cublasDestroy(handle);
    return failed;
  });
}

extern "C" int dx_cublas_kernel_gpu(int n, float a, const float* x, float* y) {
  return RunOnDevice(n, x, y, [&](float* dx, float* dy) {
    dx_cublas_kernel<<<(n + 255) / 256, 256>>>(n, a, dx, dy);
    return 0;
  });
}

extern "C" void dx_cublas_saxpy_cpu(int n, float a, const float* x, float* y) {
  for (int i = 0; i < n; ++i) {
    y[i] = a * x[i] + y[i];
  }
}
