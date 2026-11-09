/// Shapes with an area.
module geometry.shapes;

/// A shape that has an area.
interface Shape
{
    /// The area of the shape.
    double area() const;
}

/// A circle.
class Circle : Shape
{
    /// The radius.
    double radius;

    /// Makes a circle.
    ///
    /// Params:
    ///     radius = the radius of the circle
    this(double radius)
    {
        this.radius = radius;
    }

    /// The area of the circle.
    override double area() const
    {
        return 3.14159 * radius * radius;
    }

    private void scratch() {}
}

/// The ratio of a circle's circumference to its diameter.
enum pi = 3.14159;

/// Scales a value.
///
/// Params:
///     value = the value to scale
///     factor = the scale, one by default
///
/// Returns: the scaled value
double scale(double value, double factor = 1.0) pure
{
    return value * factor;
}
