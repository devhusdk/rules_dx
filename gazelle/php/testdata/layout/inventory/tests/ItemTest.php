<?php

use Inventory\Item;
use PHPUnit\Framework\TestCase;

final class ItemTest extends TestCase
{
    public function testItem(): void
    {
        $this->assertInstanceOf(Item::class, new Item());
    }
}
