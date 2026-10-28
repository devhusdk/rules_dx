<?php

namespace Acme\App\Tests;

use Acme\App\Helper;
use PHPUnit\Framework\TestCase;

final class MainTest extends TestCase
{
    public function testHelper(): void
    {
        $this->assertInstanceOf(Helper::class, new Helper());
    }
}
