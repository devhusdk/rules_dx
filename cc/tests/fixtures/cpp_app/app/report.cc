#include "cc/tests/fixtures/cpp_app/app/report.h"

#include "cc/tests/fixtures/cpp_app/core/group.h"
#include "fmt/format.h"

namespace pantry {

std::string render_report(const std::vector<Dish> &dishes) {
  std::string out;
  for (const Dish &dish : dishes) {
    out += fmt::format("{:<10}{:>9}{:>8.2f}\n", dish.name, dish.category,
                       static_cast<double>(dish.price_cents) / 100.0);
  }
  const auto totals = group_sum(
      dishes, [](const Dish &dish) { return dish.category; },
      [](const Dish &dish) { return dish.price_cents; });
  for (const auto &[category, cents] : totals) {
    out += fmt::format("total {} {}\n", category, cents);
  }
  return out;
}

} // namespace pantry
