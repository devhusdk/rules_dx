#include "cuda/tests/fixtures/rdc/scale.cuh"

__device__ float dx_rdc_scale(float a, float x) {
  return a * x;
}
