#ifndef DX_CUDA_TESTING_GPU_DEVICE_H_
#define DX_CUDA_TESTING_GPU_DEVICE_H_

#ifdef __cplusplus
extern "C" {
#endif

const char *dx_gpu_device_compute_capability(void);

const char *dx_gpu_device_driver(void);

#ifdef __cplusplus
}
#endif

#endif
