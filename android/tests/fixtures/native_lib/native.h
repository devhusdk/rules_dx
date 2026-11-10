#ifndef NATIVE_LIB_H_
#define NATIVE_LIB_H_

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if !defined(NATIVE_OFFSET)
#error "native copts did not reach the compile"
#endif

int32_t native_add(int32_t left, int32_t right);
int32_t native_offset(void);

#ifdef __cplusplus
}
#endif

#endif
