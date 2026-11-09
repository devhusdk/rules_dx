--- Geometry helpers for the docs fixture.
-- Areas of simple shapes.
-- @module shapes

local shapes = {}

--- The area of a circle.
-- @tparam number radius Radius of the circle.
-- @treturn number The area.
-- @usage local a = shapes.circle_area(2)
function shapes.circle_area(radius)
  return math.pi * radius * radius
end

--- The area of a rectangle.
-- @param width Width of the rectangle.
-- @param height Height of the rectangle.
-- @return The area.
function shapes.rect_area(width, height)
  return width * height
end

--- Default unit of every length.
-- @field name Name of the unit.
shapes.unit = { name = "m" }

local function hidden() end

return shapes
