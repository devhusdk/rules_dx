// Seed GoogleTest case for a cc_test with no srcs of its own.
#include <gtest/gtest.h>

TEST(NoSrcsTest, Passes) { EXPECT_EQ(1 + 1, 2); }
