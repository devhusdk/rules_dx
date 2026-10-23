#include "greet.h"

__declspec(dllexport) const char *greet_line(void) {
  return "greeting from greet.dll";
}
