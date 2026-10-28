package dx.fixture.shapes

/** A circle given by its radius.
  *
  * Use [[Circle.scale(factor:Double)* scale]] to stretch it.
  *
  * @param radius
  *   the radius, never negative
  */
class Circle(val radius: Double) extends Shape {

  /** The size classes a circle can have. */
  type Size = Int

  private val hidden: Double = radius

  private[shapes] def inside(): Double = hidden

  override def area(): Double = 3 * radius * radius

  /** Scales the radius by one factor.
    *
    * @param factor
    *   the factor to multiply by
    * @return
    *   the scaled circle
    */
  def scale(factor: Double): Circle = new Circle(radius * factor)

  /** Scales the radius by two factors, like [[scale(factor:Double)* scale]].
    *
    * @param horizontal
    *   the first factor
    * @param vertical
    *   the second factor
    * @return
    *   the scaled circle
    */
  def scale(horizontal: Double, vertical: Double): Circle =
    new Circle(radius * horizontal * vertical)
}

/** A point on the plane. */
case class Point(x: Double, y: Double)

/** Holds the factories of points. */
object Point {

  /** Returns the origin. */
  def origin: Point = Point(0, 0)
}
