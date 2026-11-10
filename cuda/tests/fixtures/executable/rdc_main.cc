#include <cstdio>

#include "cuda/tests/fixtures/executable/add.h"
#include "cuda/tests/fixtures/rdc/axpy.h"

int main(int argc, char **) {
  float x[4] = {1, 2, 3, 4};
  float y[4] = {1, 1, 1, 1};
  dx_rdc_axpy_cpu(4, 2.0f, x, y);
  if (argc > 100) {
    dx_exec_add_launch(0, nullptr, nullptr);
  }
  std::printf("rdc executable sum=%d\n",
              static_cast<int>(y[0] + y[1] + y[2] + y[3]));
  return y[3] == 9.0f ? 0 : 1;
}
