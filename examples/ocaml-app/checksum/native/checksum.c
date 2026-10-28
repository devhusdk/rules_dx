#include "examples/ocaml-app/checksum/native/checksum.h"

intptr_t dx_checksum_step(intptr_t h, intptr_t b) {
  return ((h ^ b) * 16777619) & 0xffffffff;
}
