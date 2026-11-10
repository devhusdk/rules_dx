<?php

declare(strict_types=1);

namespace Hello\Tests;

use PHPUnit\Framework\TestCase;

final class PcovTest extends TestCase
{
    public function testPcovIsLoadedUnderCoverageOnLinux(): void
    {
        if (getenv('COVERAGE_OUTPUT_FILE') === false || PHP_OS_FAMILY !== 'Linux') {
            $this->markTestSkipped('pcov loads only under coverage on Linux');
        }
        $this->assertTrue(extension_loaded('pcov'));
        $this->assertSame('1', ini_get('pcov.enabled'));
    }
}
