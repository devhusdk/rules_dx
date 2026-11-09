<?php

declare(strict_types=1);

namespace Geometry;

/**
 * A closed plane figure.
 */
interface Shape
{
    /**
     * Returns the enclosed area.
     *
     * @return float The area in square units.
     */
    public function area(): float;
}
