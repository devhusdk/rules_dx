#ifndef CONSUMER_NATIVE_H_
#define CONSUMER_NATIVE_H_

#include <stdint.h>

#if !defined(CONSUMER_OFFSET)
#error "consumer copts did not reach the native compile"
#endif

int32_t consumer_add(int32_t left, int32_t right);
int32_t consumer_offset(void);

#endif
