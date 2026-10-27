#ifndef DX_CUDA_CUBLAS_SAXPY_H_
#define DX_CUDA_CUBLAS_SAXPY_H_

#ifdef __cplusplus
extern "C" {
#endif

int dx_cublas_device_count(void);
/* y = a * x + y through cublasSaxpy. Returns 0 on success. */
int dx_cublas_saxpy_gpu(int n, float a, const float *x, float *y);
/* y = a * x + y through this fixture's own kernel. Returns 0 on success. */
int dx_cublas_kernel_gpu(int n, float a, const float *x, float *y);
void dx_cublas_saxpy_cpu(int n, float a, const float *x, float *y);

#ifdef __cplusplus
}
#endif

#endif
