<?php

namespace Inventory;

final class Inventory
{
    public function __construct(private Item $first, private Ledger $ledger)
    {
    }
}
