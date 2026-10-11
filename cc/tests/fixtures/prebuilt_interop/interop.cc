// Prebuilt interop fixture library; consumer of cc_library.
#include "cc/tests/fixtures/prebuilt_interop/interop.h"

#include <memory>
#include <stdexcept>
#include <string>
#include <typeinfo>
#include <vector>

std::string Circle::Name() const { return "circle"; }

std::string Join(const std::vector<std::string> &names) {
  std::string out;
  for (size_t i = 0; i < names.size(); ++i) {
    if (i > 0) {
      out += ",";
    }
    out += names[i];
  }
  return out;
}

std::string Describe(const Shape *shape) {
  if (shape == nullptr) {
    throw std::invalid_argument("null shape");
  }
  return shape->Name();
}

std::unique_ptr<Shape> MakeCircle(double radius) {
  return std::unique_ptr<Shape>(new Circle(radius));
}

bool IsCircle(const Shape *shape) {
  if (shape == nullptr) {
    return false;
  }
  if (typeid(*shape) == typeid(Circle)) {
    return true;
  }
  return dynamic_cast<const Circle *>(shape) != nullptr;
}
