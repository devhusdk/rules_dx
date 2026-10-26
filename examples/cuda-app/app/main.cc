#include <cstdio>
#include <vector>

#include "examples/cuda-app/kernels/kernels.h"

namespace {

constexpr int kN = 4096;

struct Result {
  long long add = 0;
  long long stencil = 0;
};

bool run_gpu(const std::vector<int> &a, const std::vector<int> &b,
             Result *result) {
  std::vector<int> sums(kN);
  std::vector<int> smoothed(kN);
  return dx_kernels_device_count() > 0 &&
         dx_vector_add_gpu(kN, a.data(), b.data(), sums.data()) == 0 &&
         dx_stencil_gpu(kN, sums.data(), smoothed.data()) == 0 &&
         dx_reduce_sum_gpu(kN, sums.data(), &result->add) == 0 &&
         dx_reduce_sum_gpu(kN, smoothed.data(), &result->stencil) == 0;
}

Result run_cpu(const std::vector<int> &a, const std::vector<int> &b) {
  std::vector<int> sums(kN);
  std::vector<int> smoothed(kN);
  dx_vector_add_cpu(kN, a.data(), b.data(), sums.data());
  dx_stencil_cpu(kN, sums.data(), smoothed.data());
  Result result;
  result.add = dx_reduce_sum_cpu(kN, sums.data());
  result.stencil = dx_reduce_sum_cpu(kN, smoothed.data());
  return result;
}

} // namespace

int main() {
  std::vector<int> a(kN);
  std::vector<int> b(kN);
  for (int i = 0; i < kN; ++i) {
    a[i] = i;
    b[i] = 2 * i;
  }
  Result result;
  if (!run_gpu(a, b, &result)) {
    result = run_cpu(a, b);
  }
  std::printf("cuda-app n=%d add=%lld stencil=%lld\n", kN, result.add,
              result.stencil);
  return 0;
}
