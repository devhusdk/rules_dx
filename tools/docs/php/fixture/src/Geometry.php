<?php
declare(strict_types=1);

namespace Geometry;

/**
 * A shape with an area.
 */
interface Shape
{
    /**
     * Returns the area.
     */
    public function area(): float;
}

/**
 * A circle.
 */
final class Circle implements Shape
{
    public function __construct(private float $radius)
    {
    }

    /**
     * Returns the area of the circle.
     */
    public function area(): float
    {
        return M_PI * $this->radius ** 2;
    }

    private function hidden(): void
    {
    }
}

/**
 * Builds a unit circle.
 */
function unit(): Circle
{
    $inner = function () {
        return 1;
    };
    return new Circle((float) $inner());
}
