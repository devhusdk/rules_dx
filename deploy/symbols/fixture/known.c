#include <stdio.h>

__attribute__((noinline)) int dx_known_function(int x) { return x * 7 + 3; }

int main(void) {
  printf("%d\n", dx_known_function(6));
  return 0;
}
