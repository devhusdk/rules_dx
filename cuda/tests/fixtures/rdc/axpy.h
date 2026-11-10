#ifndef DX_CUDA_RDC_AXPY_H_
#define DX_CUDA_RDC_AXPY_H_

#ifdef __cplusplus
extern "C" {
#endif

int dx_rdc_device_count(void);
int dx_rdc_axpy_gpu(int n, float a, const float *x, float *y);
void dx_rdc_axpy_cpu(int n, float a, const float *x, float *y);

#ifdef __cplusplus
}
#endif

#endif
