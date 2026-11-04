#include <stdio.h>

#include "add.h"

#ifndef _MSC_VER
#error "expected MSVC"
#endif

#ifdef __clang__
#error "expected cl.exe, not clang"
#endif

int main(void) {
  printf("_MSC_FULL_VER=%lu\n", (unsigned long)_MSC_FULL_VER);
  return dx_add(2, 3) == 5 ? 0 : 1;
}
