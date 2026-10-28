<?php

use Inventory\Ledger;
use PHPUnit\Framework\TestCase;

final class LedgerTest extends TestCase
{
    public function testCount(): void
    {
        $this->assertCount(0, new Ledger());
    }
}
