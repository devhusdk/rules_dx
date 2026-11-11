#ifndef CUDA_PRIVATE_GPU_GPU_GUARD_H_
#define CUDA_PRIVATE_GPU_GPU_GUARD_H_

#ifdef __cplusplus
extern "C" {
#endif

void dx_gpu_install_fault_handler(void);
unsigned long dx_gpu_guarded(void (*func)(void *), void *ctx);

#ifdef __cplusplus
}
#endif

#endif // CUDA_PRIVATE_GPU_GPU_GUARD_H_
