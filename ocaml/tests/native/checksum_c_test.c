#include <stdio.h>

#include "ocaml/tests/fixtures/ocaml-app/checksum/native/checksum.h"

int main(void) {
  intptr_t h = dx_checksum_step(2166136261, 'a');
  if (h != 0xe40c292c) {
    fprintf(stderr, "dx_checksum_step('a') = %lx\n", (unsigned long)h);
    return 1;
  }
  return 0;
}
