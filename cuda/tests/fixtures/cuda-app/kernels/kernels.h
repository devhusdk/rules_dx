#ifndef DX_CUDA_APP_KERNELS_H_
#define DX_CUDA_APP_KERNELS_H_

#ifdef __cplusplus
extern "C" {
#endif

int dx_kernels_device_count(void);
int dx_vector_add_gpu(int n, const int *a, const int *b, int *out);
void dx_vector_add_cpu(int n, const int *a, const int *b, int *out);
int dx_reduce_sum_gpu(int n, const int *x, long long *sum);
long long dx_reduce_sum_cpu(int n, const int *x);
int dx_stencil_gpu(int n, const int *in, int *out);
void dx_stencil_cpu(int n, const int *in, int *out);

#ifdef __cplusplus
}
#endif

#endif
