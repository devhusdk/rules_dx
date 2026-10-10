#include <stdio.h>

#include "hello.h"

char *Hello(char *out, size_t size, const char *name) {
  if (out == NULL || size == 0) {
    return NULL;
  }
  if (name == NULL) {
    name = "world";
  }
  snprintf(out, size, "hello %s", name);
  return out;
}
