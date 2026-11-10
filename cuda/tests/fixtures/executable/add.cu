#include "tests/fixtures/executable/add.h"

__global__ void dx_exec_add_kernel(int n, const int *x, int *y) {
  int i = blockIdx.x * blockDim.x + threadIdx.x;
  if (i < n) {
    y[i] += x[i];
  }
}

extern "C" int dx_exec_add_launch(int n, const int *x, int *y) {
  dx_exec_add_kernel<<<(n + 255) / 256, 256>>>(n, x, y);
  return 0;
}
