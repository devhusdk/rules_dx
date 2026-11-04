#include <cstdio>
#include <string>

#include "add.h"
#include "greet.h"

#ifndef _MSC_VER
#error "expected MSVC"
#endif

#ifdef __clang__
#error "expected cl.exe, not clang"
#endif

int main() {
  std::printf("_MSC_FULL_VER=%lu\n", static_cast<unsigned long>(_MSC_FULL_VER));
  if (dx_greet_dll_msc_full_ver() != _MSC_FULL_VER) {
    std::fprintf(stderr, "dll compiler differs\n");
    return 1;
  }
  if (dx_greet_dll("a") != "dll a" || dx_greet_static("a") != "hello a!") {
    return 1;
  }
  return dx_add(1, 2) == 3 ? 0 : 1;
}
