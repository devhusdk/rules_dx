#include <cuda_runtime.h>

#include "cuda/tests/fixtures/rdc/axpy.h"
#include "cuda/tests/fixtures/rdc/scale.cuh"

__global__ void dx_rdc_axpy_kernel(int n, float a, const float* x, float* y) {
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i < n) {
    y[i] = dx_rdc_scale(a, x[i]) + y[i];
  }
}

extern "C" int dx_rdc_device_count(void) {
  int count = 0;
  if (cudaGetDeviceCount(&count) != cudaSuccess) {
    return 0;
  }
  return count;
}

extern "C" int dx_rdc_axpy_gpu(int n, float a, const float* x, float* y) {
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
  dx_rdc_axpy_kernel<<<(n + 255) / 256, 256>>>(n, a, dx, dy);
  cudaError_t status = cudaDeviceSynchronize();
  cudaMemcpy(y, dy, bytes, cudaMemcpyDeviceToHost);
  cudaFree(dx);
  cudaFree(dy);
  return status == cudaSuccess ? 0 : 1;
}

extern "C" void dx_rdc_axpy_cpu(int n, float a, const float* x, float* y) {
  for (int i = 0; i < n; ++i) {
    y[i] = a * x[i] + y[i];
  }
}
