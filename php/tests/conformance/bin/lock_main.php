<?php

declare(strict_types=1);

echo interface_exists(Psr\Http\Message\RequestFactoryInterface::class) ? "hello lock\n" : "missing\n";
