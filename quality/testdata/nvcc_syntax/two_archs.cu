#include "quality/testdata/nvcc_syntax/dep.cuh"

__global__ void two_archs_kernel(int *out) {
  int unused = 0;
  *out = dep_value() + CLEAN_OFFSET;
}
