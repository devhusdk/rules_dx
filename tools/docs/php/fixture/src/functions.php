<?php

declare(strict_types=1);

namespace Geometry;

/**
 * Sums the areas of shapes.
 *
 * @param Shape ...$shapes The shapes to add up.
 * @return float The total area.
 */
function total_area(Shape ...$shapes): float
{
    return array_sum(array_map(static fn (Shape $shape): float => $shape->area(), $shapes));
}
