"""
Geometric shapes.
"""
module Shapes

export Shape, Circle, area, scale, UNIT, @unit

"""
A shape with an area.
"""
abstract type Shape end

"""
A circle given by its radius.
"""
struct Circle <: Shape
    radius::Float64
end

"""
    area(c::Circle)

Returns the area of the circle.
"""
area(c::Circle) = pi * c.radius^2

"""
Scales a circle by `factor`.
"""
scale(c::Circle, factor::Real) = Circle(c.radius * factor)

"""
The unit circle.
"""
const UNIT = Circle(1.0)

"""
Builds the unit circle.
"""
macro unit()
    return :(Circle(1.0))
end

# Not exported: stays out of the API.
hidden(c::Circle) = c.radius

end # module
