#include "pkg/gpuonly/kernel.cuh"

__global__ void TwiceAll(float* values, int n) {
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i < n) {
    values[i] = Twice(values[i]);
  }
}
