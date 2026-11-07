#ifndef DX_EXEC_PUBLIC
#error "defines did not reach nvcc"
#endif

#ifndef DX_EXEC_N
#error "local_defines did not reach nvcc"
#endif

#ifndef DX_EXEC_NVCC
#error "cuda_copts did not reach nvcc"
#endif

static_assert(DX_EXEC_N == 1024, "DX_EXEC_N");

int dx_defines_value() {
    return DX_EXEC_N;
}
