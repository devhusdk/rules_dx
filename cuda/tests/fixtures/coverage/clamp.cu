#include "cuda/tests/fixtures/coverage/clamp.h"

__global__ void dx_cov_kernel(int *out) { out[0] = 1; }

int dx_cov_clamp(int value, int low, int high) {
  if (value < low) {
    return low;
  }
  if (value > high) {
    return high;
  }
  return value;
}
