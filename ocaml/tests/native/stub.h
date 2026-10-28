#ifndef DX_OCAML_STUB_H
#define DX_OCAML_STUB_H

#include <stdint.h>

/* Called from OCaml as an unboxed, untagged native external. */
intptr_t dx_add(intptr_t a, intptr_t b);

#endif
