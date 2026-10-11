// Seed C++ GoogleTest test; consumer of the qualified GoogleTest v1.18.0
// runner.
#include <gtest/gtest.h>

#include <optional>
#include <string>
#include <tuple>

#include "cc/tests/fixtures/googletest/greeter.h"

// C++17 floor proof: GoogleTest 1.18.x requires C++17 or newer per the
// upstream v1.18.0 release notes. The fixture pins `-std=c++17` in
// BUILD.bazel; this assertion fails the compile below the floor.
static_assert(__cplusplus >= 201703L, "GoogleTest v1.18.0 requires C++17");

TEST(GreeterTest, GreetsWorld) { EXPECT_EQ(Greet("world"), "hello world"); }

TEST(GreeterTest, MaybeGreetEmptyIsNullopt) {
  EXPECT_FALSE(MaybeGreet("").has_value());
  ASSERT_TRUE(MaybeGreet("world").has_value());
  EXPECT_EQ(MaybeGreet("world").value(), "hello world");
}

TEST(GreeterTest, StructuredBindingsFloor) {
  // Structured bindings are C++17-only; this fails to compile below C++17.
  auto [greeted, ok] = std::make_tuple(Greet("world"), true);
  EXPECT_TRUE(ok);
  EXPECT_EQ(greeted, "hello world");
}

TEST(GreeterTest, IfConstexprFloor) {
  // if constexpr is C++17-only; this fails to compile below C++17.
  if constexpr (__cplusplus >= 201703L) {
    EXPECT_EQ(Greet("dx"), "hello dx");
  } else {
    FAIL() << "below the C++17 floor";
  }
}
