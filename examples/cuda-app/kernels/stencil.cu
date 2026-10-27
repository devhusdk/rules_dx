#include "examples/cuda-app/kernels/common.cuh"
#include "examples/cuda-app/kernels/kernels.h"

__global__ void dx_stencil_kernel(int n, const int *in, int *out) {
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i < n) {
    out[i] = dx_stencil_at(n, in, i);
  }
}

extern "C" int dx_stencil_gpu(int n, const int *in, int *out) {
  int *din = dx_device_copy(n, in);
  int *dout = dx_device_copy(n, nullptr);
  int status = 1;
  if (din != nullptr && dout != nullptr) {
    dx_stencil_kernel<<<dx_grid(n), kDxBlock>>>(n, din, dout);
    if (cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(out, dout, dx_bytes(n), cudaMemcpyDeviceToHost) ==
            cudaSuccess) {
      status = 0;
    }
  }
  cudaFree(din);
  cudaFree(dout);
  return status;
}

extern "C" void dx_stencil_cpu(int n, const int *in, int *out) {
  for (int i = 0; i < n; ++i) {
    out[i] = dx_stencil_at(n, in, i);
  }
}
