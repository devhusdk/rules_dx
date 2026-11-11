<?php

declare(strict_types=1);

namespace Conformance;

final class Greet
{
    public static function greet(string $name): string
    {
        return 'hello ' . $name;
    }
}
