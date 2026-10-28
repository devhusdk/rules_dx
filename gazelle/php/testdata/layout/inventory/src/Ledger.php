<?php

namespace Inventory;

use Inventory\Plugin\Base;

final class Ledger extends Base implements \Countable
{
    public function count(): int
    {
        return 0;
    }

    public function item(): Item
    {
        return new Item();
    }
}
