<?php

declare(strict_types=1);

namespace Hello\Tests;

use Hello\Format\Upper;
use Hello\Greeter;
use PHPUnit\Framework\TestCase;

final class GreeterTest extends TestCase
{
    public function testGreetsInUpperCase(): void
    {
        $this->assertSame('Hello, WORLD!', (new Greeter())->greet(" world\n"));
    }

    public function testUpperTrims(): void
    {
        $this->assertSame('PHP', (new Upper())->apply(' php '));
    }

    public function testIniReachesTheTest(): void
    {
        $this->assertSame('7', ini_get('precision'));
    }
}
