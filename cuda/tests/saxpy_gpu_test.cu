#include <cstdint>
#include <vector>

#include "cuda/testing/gpu_test.h"
#include "cuda/tests/fixtures/saxpy/saxpy.h"

namespace {

const double kAbsTol = 1e-5;
const double kRelTol = 1e-6;

float Uniform(uint32_t* state) {
  *state = *state * 1664525u + 1013904223u;
  return static_cast<float>(*state >> 8) / 16777216.0f * 4.0f - 2.0f;
}

bool Saxpy(const char* name, int n, float a, uint32_t seed) {
  std::vector<float> x(n);
  std::vector<float> y(n);
  for (int i = 0; i < n; ++i) {
    x[i] = Uniform(&seed);
    y[i] = Uniform(&seed);
  }
  std::vector<float> want = y;
  std::vector<float> got = y;
  dx_saxpy_cpu(n, a, x.data(), want.data());
  if (dx_saxpy_gpu(n, a, x.data(), got.data()) != 0) {
    return dx_gpu_check(name, want.data(), nullptr, n, kAbsTol, kRelTol);
  }
  return dx_gpu_check(name, want.data(), got.data(), n, kAbsTol, kRelTol);
}

}  // namespace

extern "C" int dx_gpu_test_main(void) {
  Saxpy("saxpy_1m", 1 << 20, 2.5f, 1856u);
  Saxpy("saxpy_tail", 100003, -0.75f, 3335u);
  return 0;
}
