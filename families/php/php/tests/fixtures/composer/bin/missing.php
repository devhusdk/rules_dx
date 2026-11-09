<?php

declare(strict_types=1);

// psr/http-message is not in this binary's deps, so the autoloader cannot find the interface.
final class Orphan implements Psr\Http\Message\RequestInterface
{
}
