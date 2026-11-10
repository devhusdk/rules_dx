<?php

declare(strict_types=1);

namespace Hello;

use Hello\Format\Upper;

final class Greeter
{
    public function __construct(private readonly Upper $upper = new Upper())
    {
    }

    public function greet(string $name): string
    {
        return 'Hello, ' . $this->upper->apply($name) . '!';
    }
}
