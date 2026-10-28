#include <vector>

#include "cuda/testing/gpu_test.h"
#include "cuda/tests/fixtures/rdc/axpy.h"

namespace {

const double kAbsTol = 1e-5;
const double kRelTol = 1e-6;

bool Axpy(const char* name, int n, float a) {
  std::vector<float> x(n);
  std::vector<float> y(n);
  for (int i = 0; i < n; ++i) {
    x[i] = static_cast<float>(i % 17) - 8.0f;
    y[i] = static_cast<float>(i % 5);
  }
  std::vector<float> want = y;
  std::vector<float> got = y;
  dx_rdc_axpy_cpu(n, a, x.data(), want.data());
  if (dx_rdc_axpy_gpu(n, a, x.data(), got.data()) != 0) {
    return dx_gpu_check(name, want.data(), nullptr, n, kAbsTol, kRelTol);
  }
  return dx_gpu_check(name, want.data(), got.data(), n, kAbsTol, kRelTol);
}

}  // namespace

extern "C" int dx_gpu_test_main(void) {
  Axpy("rdc_axpy_1m", 1 << 20, 2.5f);
  Axpy("rdc_axpy_tail", 100003, -0.75f);
  return 0;
}
