package dx.fixture.paint;

import dx.fixture.shapes.Shape;

/** A square that can be painted like {@link dx.fixture.shapes.Circle}. */
public final class Square implements Shape {
  private final double side;

  /**
   * Creates a square.
   *
   * @param side the side length
   */
  public Square(double side) {
    this.side = side;
  }

  @Override
  public double area() {
    return side * side;
  }
}
