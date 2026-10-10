#include <stdio.h>

#include "hello.h"

int main(void) {
  char out[64];
  puts(Hello(out, sizeof out, "world"));
  return 0;
}
