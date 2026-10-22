#ifndef EXAMPLES_CPP_APP_APP_REPORT_H_
#define EXAMPLES_CPP_APP_APP_REPORT_H_

#include <string>
#include <vector>

#include "examples/cpp-app/core/menu.h"

namespace pantry {

std::string render_report(const std::vector<Dish> &dishes);

} // namespace pantry

#endif
