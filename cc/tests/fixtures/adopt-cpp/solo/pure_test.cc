// Foreign C++ solo test: handwritten owner via pinned GoogleTest v1.18.0.
#include <gtest/gtest.h>

#include "cc/tests/fixtures/adopt-cpp/solo/pure.h"

static_assert(__cplusplus >= 201703L, "GoogleTest v1.18.0 requires C++17");

TEST(PureTest, Adds) {
  EXPECT_EQ(Add(2, 3), 5);
  EXPECT_EQ(Add(-1, 1), 0);
}
