// Prebuilt interop fixture header; STL values plus exceptions plus RTTI plus
// ownership.
#pragma once

#include <memory>
#include <string>
#include <vector>

// Base plus derived exercise RTTI across the library boundary.
class Shape {
public:
  virtual ~Shape() = default;
  virtual std::string Name() const = 0;
};

class Circle : public Shape {
public:
  explicit Circle(double radius) : radius_(radius) {}
  std::string Name() const override;
  double radius() const { return radius_; }

private:
  double radius_;
};

// Join returns the comma-joined names; STL values cross the boundary.
std::string Join(const std::vector<std::string> &names);

// Describe returns Name() or throws std::invalid_argument on null.
// Exceptions cross the boundary; the caller catches by const reference.
std::string Describe(const Shape *shape);

// MakeCircle transfers allocation ownership to the caller via unique_ptr.
// The owning side deletes; no cross-heap free is assumed.
std::unique_ptr<Shape> MakeCircle(double radius);

// IsCircle proves RTTI: dynamic_cast plus typeid work across the boundary.
bool IsCircle(const Shape *shape);
