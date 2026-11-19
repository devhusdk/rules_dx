#include "core/quality/testdata/nvcc_syntax/dep.cuh"

__host__ __device__ int tidy_dirty_value(int value) {
  if (value) {
    return 1;
  } else {
    return dep_value();
  }
}
