#include <stdio.h>

#include "ocaml/tests/fixtures/native/stub.h"

int main(void) {
  if (dx_add(2, 3) != 5 || dx_add(-4, 4) != 0) {
    (void)fputs("dx_add returned a wrong sum\n", stderr);
    return 1;
  }
  return 0;
}
