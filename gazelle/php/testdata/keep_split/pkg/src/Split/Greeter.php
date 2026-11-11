<?php

namespace Split;

use Split\Format\Upper;

final class Greeter
{
    public function __construct(private Upper $upper)
    {
    }
}
