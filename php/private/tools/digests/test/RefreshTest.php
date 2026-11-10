<?php

declare(strict_types=1);

namespace Digests\Tests;

use Digests\Refresh;
use PHPUnit\Framework\TestCase;

final class RefreshTest extends TestCase
{
    /** @var list<string> */
    private array $fetched = [];

    private function refresh(): Refresh
    {
        return new Refresh(function (string $url): string {
            $this->fetched[] = $url;
            return 'bytes of ' . $url;
        });
    }

    /**
     * @return array<string, mixed>
     */
    private function lock(): array
    {
        return [
            'packages' => [
                ['name' => 'a/changed', 'dist' => ['reference' => 'new', 'url' => 'https://x/changed.zip']],
                ['name' => 'a/same', 'dist' => ['reference' => 'ref', 'url' => 'https://x/same.zip']],
            ],
            'packages-dev' => [
                ['name' => 'b/dev', 'dist' => ['reference' => 'dev', 'url' => 'https://x/dev.zip']],
            ],
        ];
    }

    public function testChangedPackageIsRewrittenAndUnchangedKept(): void
    {
        $sidecar = ['packages' => [
            'a/changed' => ['reference' => 'old', 'sha256' => 'stale'],
            'a/same' => ['reference' => 'ref', 'sha256' => 'kept'],
            'gone/pkg' => ['reference' => 'x', 'sha256' => 'y'],
        ]];
        $out = $this->refresh()->packages($this->lock(), $sidecar)['packages'];
        $this->assertSame(['https://x/changed.zip', 'https://x/dev.zip'], $this->fetched);
        $this->assertSame(['reference' => 'new', 'sha256' => hash('sha256', 'bytes of https://x/changed.zip')], $out['a/changed']);
        $this->assertSame(['reference' => 'ref', 'sha256' => 'kept'], $out['a/same']);
        $this->assertSame(['a/changed', 'a/same', 'b/dev'], array_keys($out));
    }

    public function testCustomUrlsSurviveARewrite(): void
    {
        $sidecar = ['packages' => ['a/changed' => ['reference' => 'old', 'sha256' => 's', 'urls' => ['https://mirror/c.zip']]]];
        $out = $this->refresh()->packages($this->lock(), $sidecar)['packages'];
        $this->assertSame(['https://mirror/c.zip'], $out['a/changed']['urls']);
        $this->assertSame('new', $out['a/changed']['reference']);
    }
}
