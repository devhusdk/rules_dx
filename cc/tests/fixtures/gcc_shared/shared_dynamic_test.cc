#include <link.h>

#include <cstdio>
#include <cstring>
#include <string>

#include "greet.h"

namespace {

struct Seen {
  bool libstdcxx = false;
  bool libgcc = false;
  bool bad = false;
};

int visit(dl_phdr_info *info, size_t, void *data) {
  auto *seen = static_cast<Seen *>(data);
  const char *name = info->dlpi_name;
  bool stdcxx = std::strstr(name, "libstdc++.so.6") != nullptr;
  bool gcc = std::strstr(name, "libgcc_s.so.1") != nullptr;
  if ((stdcxx || gcc) && std::strstr(name, "_solib") == nullptr) {
    std::fprintf(stderr, "loaded from outside runfiles: %s\n", name);
    seen->bad = true;
  }
  seen->libstdcxx |= stdcxx;
  seen->libgcc |= gcc;
  return 0;
}

}  // namespace

int main() {
  if (dx_greet("world") != "hello world") {
    std::fprintf(stderr, "unexpected greeting\n");
    return 1;
  }
  Seen seen;
  dl_iterate_phdr(visit, &seen);
  if (!seen.libstdcxx || !seen.libgcc || seen.bad) {
    std::fprintf(stderr, "libstdc++=%d libgcc_s=%d bad=%d\n", seen.libstdcxx,
                 seen.libgcc, seen.bad);
    return 1;
  }
  return 0;
}
