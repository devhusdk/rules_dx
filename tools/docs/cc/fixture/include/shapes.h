/** @file shapes.h
 *  Shapes. */
#ifndef SHAPES_H
#define SHAPES_H

#ifdef __cplusplus
extern "C" {
#endif

/** Maximum number of shapes. */
#define SHAPES_MAX 64

/** Kind of a shape. */
typedef enum shape_kind {
  SHAPE_CIRCLE, /**< A circle. */
  SHAPE_SQUARE = 2 /**< A square. */
} shape_kind;

/** A point.
 */
typedef struct point {
  int x; /**< Horizontal. */
  int y; /**< Vertical. */
} point;

/** Area of a circle.
 * @param radius The radius.
 * @return The area.
 */
double circle_area(double radius);

/** Undocumented-return add. */
int add(int a, int b);

typedef unsigned long shape_id;

#ifdef __cplusplus
}
namespace geo {
/** A shape. */
class Shape {
public:
  /** Make one. @param k kind */
  explicit Shape(shape_kind k);
  /** Area. @return area */
  virtual double area() const = 0;
  /** Name. */
  int id;
private:
  int hidden;
};
}
#endif
#endif
