<?php

declare(strict_types=1);

use Fixture\RequestFactory;
use PHPUnit\Framework\TestCase;
use Psr\Http\Message\RequestFactoryInterface;

final class RequestFactoryTest extends TestCase
{
    public function testImplementsThePsrInterface(): void
    {
        $this->assertInstanceOf(RequestFactoryInterface::class, new RequestFactory());
    }

    public function testRequireEdgeAndFilesAutoload(): void
    {
        $this->assertSame('RequestFactoryInterface=loaded deprecation=files', RequestFactory::describe());
    }
}
