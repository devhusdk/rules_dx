# frozen_string_literal: true

# Shapes built on top of the geometry library.
module GeometryExt
  # A square with equal sides.
  class Square < Geometry::Shape
    include Geometry::Sized

    # Creates a square.
    #
    # @param side [Float] the length of every side.
    def initialize(side)
      @side = side
    end

    # Computes the area of the square.
    #
    # @return [Float] the area in square units.
    def area
      @side * @side
    end
  end
end
