#include <vector>

#include "cuda/testing/gpu_test.h"
#include "examples/cuda-app/kernels/kernels.h"

namespace {

std::vector<float> ToFloat(const std::vector<int> &values) {
  return std::vector<float>(values.begin(), values.end());
}

}  // namespace

extern "C" int dx_gpu_test_main(void) {
  const int n = 4096;
  std::vector<int> a(n);
  std::vector<int> b(n);
  for (int i = 0; i < n; ++i) {
    a[i] = i % 97;
    b[i] = (i * 7) % 89;
  }

  std::vector<int> want_add(n);
  std::vector<int> got_add(n);
  dx_vector_add_cpu(n, a.data(), b.data(), want_add.data());
  std::vector<float> want = ToFloat(want_add);
  if (dx_vector_add_gpu(n, a.data(), b.data(), got_add.data()) != 0) {
    dx_gpu_check("vector_add", want.data(), nullptr, n, 0.0, 0.0);
  } else {
    std::vector<float> got = ToFloat(got_add);
    dx_gpu_check("vector_add", want.data(), got.data(), n, 0.0, 0.0);
  }

  std::vector<int> want_stencil(n);
  std::vector<int> got_stencil(n);
  dx_stencil_cpu(n, want_add.data(), want_stencil.data());
  want = ToFloat(want_stencil);
  if (dx_stencil_gpu(n, want_add.data(), got_stencil.data()) != 0) {
    dx_gpu_check("stencil", want.data(), nullptr, n, 0.0, 0.0);
  } else {
    std::vector<float> got = ToFloat(got_stencil);
    dx_gpu_check("stencil", want.data(), got.data(), n, 0.0, 0.0);
  }

  float want_sum = static_cast<float>(dx_reduce_sum_cpu(n, a.data()));
  long long sum = 0;
  if (dx_reduce_sum_gpu(n, a.data(), &sum) != 0) {
    dx_gpu_check("reduce_sum", &want_sum, nullptr, 1, 0.0, 0.0);
  } else {
    float got_sum = static_cast<float>(sum);
    dx_gpu_check("reduce_sum", &want_sum, &got_sum, 1, 0.0, 0.0);
  }
  return 0;
}
