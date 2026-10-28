package dx.fixture.shapes

/**
 * A circle given by its radius, drawn with `radius` as [Double].
 *
 * Use [Circle.scale] to stretch it.<script>alert(1)</script>
 *
 * - round
 * - closed
 *
 * @param radius the radius, never negative
 * @property radius the radius that was given
 * @since 1.0
 */
open class Circle(val radius: Double) : Shape {
    /** The size classes a circle can have. */
    enum class Size {
        /** Radius below one. */
        SMALL,

        /** Radius of one or more. */
        LARGE,
    }

    /** Holds the constants and factories of circles. */
    companion object {
        /** The largest radius a circle may have. */
        const val MAX_RADIUS: Double = 100.0

        /** Returns the unit circle. */
        fun unit(): Circle = Circle(1.0)
    }

    private val hidden: Double = radius

    internal fun inside(): Double = hidden

    override fun area(): Double = 3 * radius * radius

    /**
     * Scales the radius by one factor.
     *
     * @param factor the factor to multiply by
     * @return the scaled circle
     * @throws IllegalArgumentException if the factor is negative
     */
    fun scale(factor: Double): Circle {
        require(factor >= 0) { "negative" }
        return Circle(radius * factor)
    }

    /**
     * Scales the radius by two factors, like [scale].
     *
     * @param horizontal the first factor
     * @param vertical the second factor
     * @return the scaled circle
     */
    fun scale(horizontal: Double, vertical: Double): Circle = Circle(radius * horizontal * vertical)

    /** Returns the size class of this circle. */
    fun size(): Size = if (radius < 1) Size.SMALL else Size.LARGE

    /** A point on the circle. */
    class Point(val x: Double, val y: Double)
}
