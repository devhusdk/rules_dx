<?php

declare(strict_types=1);

namespace Conformance\Tests;

use Conformance\Greet;
use PHPUnit\Framework\TestCase;

final class FailTest extends TestCase
{
    public function testCase(): void
    {
        $this->assertSame('goodbye world', Greet::greet('world'));
    }
}
