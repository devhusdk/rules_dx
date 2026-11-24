#include "adder.h"

#include <numeric>
#include <vector>

extern "C" int dx_cxx_add(int lhs, int rhs) {
  std::vector<int> values{lhs, rhs};
  return std::accumulate(values.begin(), values.end(), 0);
}
