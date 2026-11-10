#ifndef DX_CUDA_EXECUTABLE_ADD_H_
#define DX_CUDA_EXECUTABLE_ADD_H_

#ifdef __cplusplus
extern "C" {
#endif

int dx_exec_add_launch(int n, const int *x, int *y);

#ifdef __cplusplus
}
#endif

#endif
