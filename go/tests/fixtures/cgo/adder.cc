#include "adder.h"

#include <numeric>
#include <vector>

extern "C" int dx_cxx_add(int a, int b) {
  std::vector<int> values{a, b};
  return std::accumulate(values.begin(), values.end(), 0);
}
