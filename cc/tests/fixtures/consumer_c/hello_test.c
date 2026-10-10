#include <assert.h>

#include "hello.h"

int main(void) {
  assert(hello_add(40, 2) == 42);
  assert(hello_add(-1, 1) == 0);
  return 0;
}
