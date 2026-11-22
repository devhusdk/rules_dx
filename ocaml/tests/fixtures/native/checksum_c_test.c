#include <stdio.h>

#include "ocaml/tests/fixtures/ocaml_app/checksum/native/checksum.h"

int main(void) {
  intptr_t hash = dx_checksum_step(2166136261, 'a');
  if (hash != 0xe40c292c) {
    (void)fputs("dx_checksum_step('a') returned a wrong hash\n", stderr);
    return 1;
  }
  return 0;
}
