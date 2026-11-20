#include <stdio.h>

const char *greet(void);

int main(void) {
  if (puts(greet()) == EOF) {
    return 1;
  }
  return 0;
}
