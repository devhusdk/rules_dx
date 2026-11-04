#include <link.h>

#include <cstdio>
#include <cstring>
#include <stdexcept>

#include "greet.h"

#ifdef __clang__
#error "expected GCC"
#endif

static int check_lib(struct dl_phdr_info *info, size_t, void *found) {
  if (info->dlpi_name && strstr(info->dlpi_name, "libstdc++.so")) {
    *static_cast<bool *>(found) = true;
  }
  return 0;
}

int main() {
  if (strncmp(__VERSION__, "16.2.0", 6) != 0) {
    std::fprintf(stderr, "unexpected compiler: %s\n", __VERSION__);
    return 1;
  }
  if (strcmp(dx_greet_compiler(), __VERSION__) != 0) {
    std::fprintf(stderr, "library compiler: %s\n", dx_greet_compiler());
    return 1;
  }
  bool found = false;
  dl_iterate_phdr(check_lib, &found);
  if (found) {
    std::fprintf(stderr, "libstdc++.so is loaded\n");
    return 1;
  }
  try {
    throw std::runtime_error("x");
  } catch (const std::exception &) {
    return 0;
  }
  return 1;
}
