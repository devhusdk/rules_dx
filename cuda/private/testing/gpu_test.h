#ifndef DX_CUDA_TESTING_GPU_TEST_H_
#define DX_CUDA_TESTING_GPU_TEST_H_

#ifndef __cplusplus
#include <stdbool.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

int dx_gpu_test_main(void);

bool dx_gpu_check(const char *name, const float *want, const float *got, int n,
                  double abs_tol, double rel_tol);

#ifdef __cplusplus
}
#endif

#endif
