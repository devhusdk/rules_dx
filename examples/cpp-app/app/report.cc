#include "examples/cpp-app/app/report.h"

#include "examples/cpp-app/core/group.h"
#include "fmt/format.h"

namespace pantry {

std::string render_report(const std::vector<Dish>& dishes) {
  std::string out;
  for (const Dish& dish : dishes) {
    out += fmt::format("{:<10}{:>9}{:>8.2f}\n", dish.name, dish.category,
                       static_cast<double>(dish.price_cents) / 100.0);
  }
  const auto totals = group_sum(
      dishes, [](const Dish& d) { return d.category; },
      [](const Dish& d) { return d.price_cents; });
  for (const auto& [category, cents] : totals) {
    out += fmt::format("total {} {}\n", category, cents);
  }
  return out;
}

}  // namespace pantry
