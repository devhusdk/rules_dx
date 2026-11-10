// C++ IDE version entry point (proves C++ toolchain acquisition).
#include <cstdio>

int main() {
#ifdef __VERSION__
  std::printf("%s\n", __VERSION__);
#else
  std::printf("unknown compiler\n");
#endif
  return 0;
}
