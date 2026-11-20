"""
Paints shapes.
"""
module Paint

import Shapes

export Square

"""
A square given by its side.
"""
struct Square <: Shapes.Shape
    side::Float64
end

"""
Returns the area of the square.
"""
Shapes.area(s::Square) = s.side^2

end # module
