#include <cstdio>

#ifndef _MSC_VER
#error "expected MSVC"
#endif

#ifdef __clang__
#error "expected cl.exe, not clang"
#endif

int main() {
  std::printf("_MSC_FULL_VER=%lu\n", static_cast<unsigned long>(_MSC_FULL_VER));
  return 0;
}
