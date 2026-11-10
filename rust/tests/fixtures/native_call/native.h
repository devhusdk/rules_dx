#ifndef RUST_TESTS_FIXTURES_NATIVE_CALL_NATIVE_H_
#define RUST_TESTS_FIXTURES_NATIVE_CALL_NATIVE_H_

#include <stdint.h>

#if !defined(NATIVE_CALL_OFFSET)
#error "select-valued copts did not reach the native compile"
#endif

#define NATIVE_CALL_API_VERSION 1

int32_t native_add(int32_t left, int32_t right);
int32_t native_div(int32_t numer, int32_t denom, int32_t *err);
int32_t native_offset(void);

#endif
