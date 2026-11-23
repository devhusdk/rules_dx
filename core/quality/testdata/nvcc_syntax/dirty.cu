#include "core/quality/testdata/nvcc_syntax/dep.cuh"
#include "core/quality/testdata/nvcc_syntax/host_only.cuh"

// A __global__ call into a __host__ function: only a full front end catches it,
// -fdevice-syntax-only does not.
__global__ void __launch_bounds__(1) dirty_kernel(int *out) {
  *out = dep_value() + host_only();
}
