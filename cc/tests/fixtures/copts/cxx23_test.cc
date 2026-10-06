// Configurable C/C++ options fixture: the C++23 request is observable at runtime.
#include <cassert>

#include "cc/tests/fixtures/copts/cxx23.h"

int main() {
  assert(__cplusplus >= 202002L);
  assert(__cpp_multidimensional_subscript >= 202211L);
  assert(DxCxx23Standard() == static_cast<long>(__cplusplus) + 5);
  return 0;
}