# frozen_string_literal: true

module Geometry
  # A closed plane figure.
  class Shape
    # Scales the shape by a factor.
    #
    # @param factor [Numeric] the multiplier applied to every length.
    # @return [Shape] a new, scaled shape.
    def scale(factor)
      self.class.new(factor)
    end
  end

  # A circle centered on the origin.
  class Circle < Shape
    include Sized
    include Comparable

    # @return [Float] the distance from the center to the edge.
    attr_reader :radius

    # A free-form label shown next to the circle.
    attr_accessor :label

    # Creates a circle.
    #
    # @param radius [Float] the radius.
    def initialize(radius)
      @radius = radius
      @label = nil
    end

    # Computes the area of the circle.
    #
    # @return [Float] the area in square units.
    def area
      Math::PI * radius**2
    end

    # The circle of radius one.
    #
    # @return [Circle] a new unit circle.
    def self.unit
      new(1.0)
    end

    # Reports whether the circle has no area.
    #
    # @return [Boolean] true for a zero radius.
    def empty?
      radius.zero?
    end

    # Orders circles by radius.
    #
    # @param other [Circle] the circle to compare with.
    # @return [Integer] -1, 0 or 1.
    def <=>(other)
      radius <=> other.radius
    end

    def diameter
      radius * 2
    end

    # Moves the circle.
    #
    # @param offset the distance to move by.
    # @return [Circle] the moved circle.
    def shift(offset)
      self.class.new(radius + offset)
    end

    # Internal cache key.
    #
    # @api private
    # @return [String] the key.
    def cache_key
      "circle:#{radius}"
    end

    private

    # Recomputes derived values.
    #
    # @return [void]
    def recompute
      @area = area
    end
  end
end
