#include "cuda/private/gpu/gpu_guard.h"

#include "gtest/gtest.h"

#ifdef _WIN32
#include <windows.h>
#endif

namespace {

void SetFlag(void *ctx) { *static_cast<int *>(ctx) = 1; }

TEST(GpuGuardTest, NormalCallbackReturnsZero) {
  int flag = 0;
  EXPECT_EQ(dx_gpu_guarded(&SetFlag, &flag), 0UL);
  EXPECT_EQ(flag, 1);
}

#ifdef _WIN32
void RaiseAccessViolation(void *) {
  RaiseException(EXCEPTION_ACCESS_VIOLATION, 0, 0, nullptr);
}

TEST(GpuGuardTest, AccessViolationReturnsCode) {
  EXPECT_EQ(dx_gpu_guarded(&RaiseAccessViolation, nullptr), 0xC0000005UL);
}
#endif

} // namespace
