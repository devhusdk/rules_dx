#include <assert.h>
#include <string.h>

#include "hello.h"

int main(void) {
  char out[64];
  assert(strcmp(Hello(out, sizeof out, "dx"), "hello dx") == 0);
  assert(Hello(out, sizeof out, NULL) != NULL);
  assert(strcmp(out, "hello world") == 0);
  assert(Hello(NULL, sizeof out, "dx") == NULL);
  return 0;
}
