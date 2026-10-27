# frozen_string_literal: true

# Plane geometry shapes with typed measurements.
module Geometry
  # The number of dimensions every shape lives in.
  DIMENSIONS = 2

  # Something with a measurable size.
  module Sized
    # Reports whether the size is zero.
    #
    # @return [Boolean] true when the area is zero.
    def zero?
      area.zero?
    end
  end
end
