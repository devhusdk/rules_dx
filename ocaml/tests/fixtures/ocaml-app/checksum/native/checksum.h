#ifndef EXAMPLES_OCAML_APP_CHECKSUM_NATIVE_CHECKSUM_H_
#define EXAMPLES_OCAML_APP_CHECKSUM_NATIVE_CHECKSUM_H_

#include <stdint.h>

/* One FNV-1a step: folds the byte `b` into the 32-bit state `h`. */
intptr_t dx_checksum_step(intptr_t h, intptr_t b);

#endif
