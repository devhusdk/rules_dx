#include <stdio.h>

#include "ocaml/tests/native/stub.h"

int main(void) {
  if (dx_add(2, 3) != 5 || dx_add(-4, 4) != 0) {
    fprintf(stderr, "dx_add returned a wrong sum\n");
    return 1;
  }
  return 0;
}
