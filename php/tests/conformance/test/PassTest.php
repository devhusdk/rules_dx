<?php

declare(strict_types=1);

namespace Conformance\Tests;

use Conformance\Greet;
use PHPUnit\Framework\TestCase;

final class PassTest extends TestCase
{
    public function testCase(): void
    {
        $this->assertSame('hello world', Greet::greet('world'));
    }
}
