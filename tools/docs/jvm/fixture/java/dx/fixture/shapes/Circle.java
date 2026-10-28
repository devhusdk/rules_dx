package dx.fixture.shapes;

/**
 * A circle given by its radius, drawn with {@code radius} as {@code double}.
 *
 * <p>Use {@link Circle#scale(double, double)} to stretch it.<script>alert(1)</script>
 *
 * <ul>
 *   <li>round
 *   <li>closed
 * </ul>
 *
 * @since 1.0
 */
public class Circle implements Shape {
  /** The largest radius a circle may have. */
  public static final double MAX_RADIUS = 100.0;

  /** The size classes a circle can have. */
  public enum Size {
    /** Radius below one. */
    SMALL,
    /** Radius of one or more. */
    LARGE
  }

  private final double radius;

  /**
   * Creates a circle.
   *
   * @param radius the radius, never negative
   */
  public Circle(double radius) {
    this.radius = radius;
  }

  @Override
  public double area() {
    return 3 * secret(radius);
  }

  /**
   * Scales the radius by one factor.
   *
   * @param factor the factor to multiply by
   * @return the scaled circle
   * @throws IllegalArgumentException if the factor is negative
   */
  public Circle scale(double factor) {
    if (factor < 0) {
      throw new IllegalArgumentException("negative");
    }
    return new Circle(radius * factor);
  }

  /**
   * Scales the radius by two factors, like {@link #scale(double)}.
   *
   * @param horizontal the first factor
   * @param vertical the second factor
   * @return the scaled circle
   */
  public Circle scale(double horizontal, double vertical) {
    return new Circle(radius * horizontal * vertical);
  }

  /** Returns the size class of this circle. */
  public Size size() {
    return radius < 1 ? Size.SMALL : Size.LARGE;
  }

  /** Never shown, because it is protected. */
  protected void hidden() {
    // Nothing to do.
  }

  private double secret(double value) {
    return value * value;
  }
}
