package dx.fixture.paint

import dx.fixture.shapes.Circle
import dx.fixture.shapes.Shape

/** A square that is painted like a [Circle]. */
class Square(val side: Double) : Shape {
    override fun area(): Double = side * side
}
