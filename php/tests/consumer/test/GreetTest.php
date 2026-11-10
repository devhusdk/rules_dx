<?php

declare(strict_types=1);

namespace Consumer\Tests;

use Consumer\Greet;
use PHPUnit\Framework\TestCase;

final class GreetTest extends TestCase
{
    public function testAnswer(): void
    {
        $this->assertSame(42, Greet::answer());
    }
}
