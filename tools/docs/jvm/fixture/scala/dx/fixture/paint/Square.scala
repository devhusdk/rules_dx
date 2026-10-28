package dx.fixture.paint

import dx.fixture.shapes.Shape

/** A square that is painted like a [[dx.fixture.shapes.Circle Circle]]. */
class Square(val side: Double) extends Shape {
  override def area(): Double = side * side
}
