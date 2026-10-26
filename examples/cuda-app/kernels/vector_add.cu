#include "examples/cuda-app/kernels/common.cuh"
#include "examples/cuda-app/kernels/kernels.h"

__global__ void dx_vector_add_kernel(int n, const int *a, const int *b,
                                     int *out) {
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i < n) {
    out[i] = dx_add(a[i], b[i]);
  }
}

extern "C" int dx_kernels_device_count(void) {
  int count = 0;
  if (cudaGetDeviceCount(&count) != cudaSuccess) {
    return 0;
  }
  return count;
}

extern "C" int dx_vector_add_gpu(int n, const int *a, const int *b, int *out) {
  int *da = dx_device_copy(n, a);
  int *db = dx_device_copy(n, b);
  int *dout = dx_device_copy(n, nullptr);
  int status = 1;
  if (da != nullptr && db != nullptr && dout != nullptr) {
    dx_vector_add_kernel<<<dx_grid(n), kDxBlock>>>(n, da, db, dout);
    if (cudaGetLastError() == cudaSuccess &&
        cudaDeviceSynchronize() == cudaSuccess &&
        cudaMemcpy(out, dout, dx_bytes(n), cudaMemcpyDeviceToHost) ==
            cudaSuccess) {
      status = 0;
    }
  }
  cudaFree(da);
  cudaFree(db);
  cudaFree(dout);
  return status;
}

extern "C" void dx_vector_add_cpu(int n, const int *a, const int *b, int *out) {
  for (int i = 0; i < n; ++i) {
    out[i] = dx_add(a[i], b[i]);
  }
}
