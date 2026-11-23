#include "core/quality/testdata/nvcc_syntax/dep.cuh"

__global__ void __launch_bounds__(1) two_archs_kernel(int *out) {
  int unused = 0;
  *out = dep_value() + CLEAN_OFFSET;
}
