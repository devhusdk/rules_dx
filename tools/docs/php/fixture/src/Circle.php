<?php

declare(strict_types=1);

namespace Geometry;

/**
 * A circle around a centre.
 */
final class Circle implements Shape
{
    /** The ratio of a circle's circumference to its diameter. */
    public const PI = 3.14159;

    /** The radius in units. */
    public float $radius;

    /**
     * Creates a circle.
     *
     * @param float $radius The radius in units.
     */
    public function __construct(float $radius)
    {
        $this->radius = $radius;
    }

    /**
     * Returns the enclosed area.
     *
     * @return float The area in square units.
     */
    public function area(): float
    {
        return self::PI * $this->radius ** 2;
    }

    /**
     * Scales a circle.
     *
     * @param float $factor The scale factor.
     * @return self The scaled circle.
     */
    public function scale(float $factor = 1.0): self
    {
        return new self($this->radius * $factor);
    }

    /** @internal */
    public function debug(): string
    {
        return 'circle';
    }

    private function secret(): void
    {
    }
}
