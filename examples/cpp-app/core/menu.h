#ifndef EXAMPLES_CPP_APP_CORE_MENU_H_
#define EXAMPLES_CPP_APP_CORE_MENU_H_

#include <istream>
#include <stdexcept>
#include <string>
#include <vector>

namespace pantry {

struct Dish {
  std::string name;
  std::string category;
  long price_cents;
};

class ParseError : public std::runtime_error {
 public:
  using std::runtime_error::runtime_error;
};

std::vector<Dish> parse_menu(std::istream& in);
std::vector<Dish> load_menu(const std::string& path);
int menu_version();

}  // namespace pantry

#endif
