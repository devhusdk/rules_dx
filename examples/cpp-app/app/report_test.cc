#include "examples/cpp-app/app/report.h"

#include "gtest/gtest.h"

namespace pantry {
namespace {

TEST(ReportTest, FormatsRowsAndTotals) {
  const std::vector<Dish> dishes = {{"soup", "starter", 650},
                                    {"steak", "main", 2350},
                                    {"bread", "starter", 300}};
  const std::string expected = "soup        starter    6.50\n"
                               "steak          main   23.50\n"
                               "bread       starter    3.00\n"
                               "total main 2350\n"
                               "total starter 950\n";
  EXPECT_EQ(render_report(dishes), expected);
}

TEST(ReportTest, PackagedMenuReport) {
  const auto report =
      render_report(load_menu("examples/cpp-app/core/menu.csv"));
  EXPECT_NE(report.find("tiramisu"), std::string::npos);
  EXPECT_NE(report.find("total dessert 790\n"), std::string::npos);
}

} // namespace
} // namespace pantry
