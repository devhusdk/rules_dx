#ifndef DX_CUDA_SAXPY_CUH_
#define DX_CUDA_SAXPY_CUH_

__device__ __host__ inline float dx_saxpy_one(float a, float x, float y) {
  return a * x + y;
}

#endif
