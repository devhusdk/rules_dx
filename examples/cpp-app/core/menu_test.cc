#include "examples/cpp-app/core/menu.h"

#include <sstream>

#include "examples/cpp-app/core/group.h"
#include "gtest/gtest.h"

namespace pantry {
namespace {

static_assert(__cplusplus >= 201703L, "cpp-app builds as C++17 or newer");

TEST(MenuTest, ParsesRows) {
  std::istringstream in("name,category,price_cents\nsoup,starter,650\n");
  const auto dishes = parse_menu(in);
  ASSERT_EQ(dishes.size(), 1u);
  EXPECT_EQ(dishes[0].name, "soup");
  EXPECT_EQ(dishes[0].price_cents, 650);
}

TEST(MenuTest, RejectsBadPrice) {
  std::istringstream in("name,category,price_cents\nsoup,starter,6x\n");
  EXPECT_THROW(parse_menu(in), ParseError);
}

TEST(MenuTest, RejectsMissingHeader) {
  std::istringstream in("soup,starter,650\n");
  EXPECT_THROW(parse_menu(in), ParseError);
}

TEST(MenuTest, RejectsWrongColumnCount) {
  std::istringstream in("name,category,price_cents\nsoup,starter\n");
  EXPECT_THROW(parse_menu(in), ParseError);
}

TEST(MenuTest, RejectsNonNumericPrice) {
  std::istringstream in("name,category,price_cents\nsoup,starter,abc\n");
  EXPECT_THROW(parse_menu(in), ParseError);
}

TEST(MenuTest, RejectsMissingFile) {
  EXPECT_THROW(load_menu("examples/cpp-app/core/absent.csv"), ParseError);
}

TEST(MenuTest, GeneratedHeaderCarriesVersion) { EXPECT_EQ(menu_version(), 3); }

TEST(MenuTest, LoadsPackagedResource) {
  const auto dishes = load_menu("examples/cpp-app/core/menu.csv");
  EXPECT_EQ(dishes.size(), 5u);
}

TEST(GroupTest, SumsByCategoryWithTemplates) {
  const auto dishes = load_menu("examples/cpp-app/core/menu.csv");
  const auto totals = group_sum(
      dishes, [](const Dish &d) { return d.category; },
      [](const Dish &d) { return d.price_cents; });
  ASSERT_EQ(totals.size(), 3u);
  EXPECT_EQ(totals.at("starter"), 1370);
  EXPECT_EQ(totals.at("main"), 3830);
  EXPECT_EQ(totals.at("dessert"), 790);
}

TEST(GroupTest, WorksForOtherRowTypes) {
  const std::vector<int> numbers = {1, 2, 3, 4, 5};
  const auto totals = group_sum(
      numbers, [](int n) { return n % 2 == 0; }, [](int n) { return n * 1.5; });
  EXPECT_DOUBLE_EQ(totals.at(true), 9.0);
  EXPECT_DOUBLE_EQ(totals.at(false), 13.5);
}

} // namespace
} // namespace pantry
