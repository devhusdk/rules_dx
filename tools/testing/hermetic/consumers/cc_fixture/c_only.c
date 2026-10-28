#include <stdlib.h>

int class_slot(void) {
  int class = 1;
  int *slot = malloc(sizeof *slot);
  if (slot == NULL) {
    return 0;
  }
  *slot = class;
  class = *slot;
  free(slot);
  return class;
}
