#include <cstdio>
#include <vector>

#include "cuda/tests/fixtures/cublas/cublas_saxpy.h"

int main() {
  const int n = 1024;
  std::vector<float> x(n);
  std::vector<float> y(n);
  for (int i = 0; i < n; ++i) {
    x[i] = static_cast<float>(i);
    y[i] = 1.0f;
  }
  if (dx_cublas_device_count() == 0 ||
      dx_cublas_saxpy_gpu(n, 2.0f, x.data(), y.data()) != 0) {
    for (int i = 0; i < n; ++i) {
      y[i] = 1.0f;
    }
    dx_cublas_saxpy_cpu(n, 2.0f, x.data(), y.data());
  }
  long long sum = 0;
  for (int i = 0; i < n; ++i) {
    sum += static_cast<long long>(y[i]);
  }
  std::printf("cublas saxpy n=%d sum=%lld\n", n, sum);
  return 0;
}
