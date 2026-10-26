#include "examples/cuda-app/kernels/common.cuh"
#include "examples/cuda-app/kernels/kernels.h"

__global__ void dx_reduce_kernel(int n, const int *x, unsigned long long *sum) {
  __shared__ long long partial[kDxBlock];
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  partial[threadIdx.x] = i < n ? x[i] : 0;
  __syncthreads();
  for (int stride = blockDim.x / 2; stride > 0; stride /= 2) {
    if (threadIdx.x < stride) {
      partial[threadIdx.x] += partial[threadIdx.x + stride];
    }
    __syncthreads();
  }
  if (threadIdx.x == 0) {
    atomicAdd(sum, static_cast<unsigned long long>(partial[0]));
  }
}

extern "C" int dx_reduce_sum_gpu(int n, const int *x, long long *sum) {
  int *dx = dx_device_copy(n, x);
  unsigned long long *dsum = nullptr;
  int status = 1;
  if (dx != nullptr && cudaMalloc(&dsum, sizeof(*dsum)) == cudaSuccess &&
      cudaMemset(dsum, 0, sizeof(*dsum)) == cudaSuccess) {
    dx_reduce_kernel<<<dx_grid(n), kDxBlock>>>(n, dx, dsum);
    unsigned long long host = 0;
    if (cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(&host, dsum, sizeof(host), cudaMemcpyDeviceToHost) ==
            cudaSuccess) {
      *sum = static_cast<long long>(host);
      status = 0;
    }
  }
  cudaFree(dx);
  cudaFree(dsum);
  return status;
}

extern "C" long long dx_reduce_sum_cpu(int n, const int *x) {
  long long sum = 0;
  for (int i = 0; i < n; ++i) {
    sum += x[i];
  }
  return sum;
}
