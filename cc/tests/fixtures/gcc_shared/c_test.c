#include <stdio.h>
#include <string.h>

#ifdef __clang__
#error "expected GCC"
#endif

int main(void) {
  if (strncmp(__VERSION__, "16.2.0", 6) != 0) {
    fprintf(stderr, "unexpected compiler: %s\n", __VERSION__);
    return 1;
  }
  return 0;
}
