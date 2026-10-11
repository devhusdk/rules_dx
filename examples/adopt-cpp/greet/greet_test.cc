// Foreign C++ test: handwritten owner of the test-owned source. Runs via
// the pinned GoogleTest v1.18.0 hub (@googletest//:gtest_main) with an
// explicit -std=c++17 floor; angle includes stay inert to generation so no
// resolve mapping is needed and the rule survives regeneration unchanged.
#include <gtest/gtest.h>

#include "examples/adopt-cpp/greet/greet.h"
#include "examples/adopt-cpp/greet/helper.h"

// C++17 floor proof: GoogleTest 1.18.x requires C++17 or newer; this
// assertion fails the compile below the floor.
static_assert(__cplusplus >= 201703L, "GoogleTest v1.18.0 requires C++17");

TEST(GreetTest, GreetsWorld) { EXPECT_EQ(Greet("dx"), "hello dx world"); }

TEST(GreetTest, HelperSuffix) { EXPECT_EQ(HelperSuffix(), " world"); }
