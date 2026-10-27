<?php

declare(strict_types=1);

namespace Fixture;

use Psr\Http\Message\RequestFactoryInterface;
use Psr\Http\Message\RequestInterface;

final class RequestFactory implements RequestFactoryInterface
{
    public function createRequest(string $method, $uri): RequestInterface
    {
        throw new \LogicException('the fixture only checks that the PSR interfaces load');
    }

    public static function describe(): string
    {
        return 'RequestFactoryInterface=' . (interface_exists(RequestInterface::class) ? 'loaded' : 'missing')
            . ' deprecation=' . (function_exists('trigger_deprecation') ? 'files' : 'missing');
    }
}
