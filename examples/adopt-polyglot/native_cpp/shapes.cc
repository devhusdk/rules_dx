#include "examples/adopt-polyglot/native_cpp/shapes.h"

double Area(const std::string& shape) {
  if (shape == "square") {
    return 4.0;
  }
  if (shape == "triangle") {
    return 2.0;
  }
  return 0.0;
}
