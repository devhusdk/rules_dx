#include "quality/testdata/nvcc_syntax/dep.cuh"

__global__ void clean_kernel(int *out) { *out = dep_value() + CLEAN_OFFSET; }
