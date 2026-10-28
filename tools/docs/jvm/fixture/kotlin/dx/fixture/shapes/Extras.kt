package dx.fixture.shapes

/** A named colour of a shape. */
data class Tint(val name: String, val level: Int)

/** Registers shapes by name. */
object Registry {
    /** Returns how many shapes are registered. */
    fun count(): Int = 0
}

/**
 * Grows a circle by one amount.
 *
 * @param by the amount to add to the radius
 * @return the larger circle
 */
fun Circle.grow(by: Double): Circle = Circle(radius + by)

/** The diameter of a circle. */
val Circle.diameter: Double
    get() = radius * 2

/** The diameter of a square-sided shape. */
val Shape.diameter: Double
    get() = 0.0

/** A function for shapes that is not public. */
internal fun secret(): Int = 1

private fun hiddenHelper(): Int = 2
