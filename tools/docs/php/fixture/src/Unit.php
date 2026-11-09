<?php

declare(strict_types=1);

namespace Geometry;

/**
 * Units of length.
 */
enum Unit: string
{
    /** Metres. */
    case Metre = 'm';

    /** Feet. */
    case Foot = 'ft';
}
