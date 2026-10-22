#include "examples/cpp-app/core/menu.h"

#include <fstream>
#include <sstream>

#include "examples/cpp-app/core/build_config.h"

namespace pantry {

namespace {

std::vector<std::string> split(const std::string &line) {
  std::vector<std::string> cells;
  std::stringstream stream(line);
  std::string cell;
  while (std::getline(stream, cell, ',')) {
    cells.push_back(cell);
  }
  return cells;
}

} // namespace

std::vector<Dish> parse_menu(std::istream &in) {
  std::string line;
  if (!std::getline(in, line) || line != "name,category,price_cents") {
    throw ParseError("menu header is missing");
  }
  std::vector<Dish> dishes;
  int number = 1;
  while (std::getline(in, line)) {
    ++number;
    const auto cells = split(line);
    if (cells.size() != 3) {
      throw ParseError("line " + std::to_string(number) +
                       ": expected 3 columns");
    }
    std::size_t used = 0;
    long price = 0;
    try {
      price = std::stol(cells[2], &used);
    } catch (const std::logic_error &) {
      used = 0;
    }
    if (used != cells[2].size()) {
      throw ParseError("line " + std::to_string(number) + ": bad price");
    }
    dishes.push_back({cells[0], cells[1], price});
  }
  return dishes;
}

std::vector<Dish> load_menu(const std::string &path) {
  std::ifstream file(path);
  if (!file) {
    throw ParseError("cannot open " + path);
  }
  return parse_menu(file);
}

int menu_version() { return kMenuVersion; }

} // namespace pantry
