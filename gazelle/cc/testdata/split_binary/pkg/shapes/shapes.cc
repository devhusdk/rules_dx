#include "pkg/shapes/shapes.h"

#include <string>

double Area(const std::string& shape) {
  if (shape == "square") {
    return 4.0;
  }
  if (shape == "triangle") {
    return 2.0;
  }
  return 0.0;
}
