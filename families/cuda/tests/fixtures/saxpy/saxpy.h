#ifndef DX_CUDA_SAXPY_H_
#define DX_CUDA_SAXPY_H_

#ifdef __cplusplus
extern "C" {
#endif

int dx_saxpy_device_count(void);
int dx_saxpy_gpu(int n, float a, const float *x, float *y);
void dx_saxpy_cpu(int n, float a, const float *x, float *y);

#ifdef __cplusplus
}
#endif

#endif
