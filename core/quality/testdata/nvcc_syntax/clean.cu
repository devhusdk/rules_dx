#include "core/quality/testdata/nvcc_syntax/dep.cuh"

__global__ void __launch_bounds__(1) clean_kernel(int *out) {
  *out = dep_value() + CLEAN_OFFSET;
}
