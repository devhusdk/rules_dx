#include <stdio.h>
#include <windows.h>

int tidy_crt_value(void) {
  return (int)sizeof(DWORD) + (int)sizeof(FILE *);
}
