// Configurable C/C++ options fixture: the requested C++ standard is never translated down.
#include "cc/tests/fixtures/copts/cxx23.h"

#if __cplusplus < 202002L
#error "the requested C++ standard was translated down"
#endif

#if !defined(__cpp_multidimensional_subscript)
#error "the C++23 multidimensional subscript operator is unavailable"
#endif

namespace {

struct DxGrid {
  int cells[4];

  int& operator[](int row, int column) { return cells[row * 2 + column]; }
};

}  // namespace

long DxCxx23Standard() {
  DxGrid grid{};
  grid[1, 1] = 5;
  return static_cast<long>(__cplusplus) + grid[1, 1];
}