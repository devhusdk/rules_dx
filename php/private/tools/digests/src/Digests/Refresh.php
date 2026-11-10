<?php

declare(strict_types=1);

namespace Digests;

use Closure;
use RuntimeException;

final class Refresh
{
    /**
     * @param Closure(string): string $fetch returns the bytes behind a URL
     */
    public function __construct(private readonly Closure $fetch)
    {
    }

    /**
     * Returns the sidecar for a lock: unchanged references keep their entry, changed ones are downloaded.
     *
     * @param array<string, mixed> $lock
     * @param array<string, mixed> $sidecar
     * @return array<string, mixed>
     */
    public function packages(array $lock, array $sidecar): array
    {
        $old = $sidecar['packages'] ?? [];
        $packages = [];
        foreach (['packages', 'packages-dev'] as $group) {
            foreach ($lock[$group] ?? [] as $entry) {
                $name = $entry['name'];
                $dist = $entry['dist'] ?? null;
                if ($dist === null) {
                    continue;
                }
                $reference = $dist['reference'] ?? '';
                $kept = $old[$name] ?? [];
                if (($kept['reference'] ?? null) === $reference) {
                    $packages[$name] = $kept;
                    continue;
                }
                $next = ['reference' => $reference, 'sha256' => hash('sha256', ($this->fetch)($dist['url']))];
                if (isset($kept['urls'])) {
                    $next['urls'] = $kept['urls'];
                }
                $packages[$name] = $next;
            }
        }
        ksort($packages);
        return ['packages' => $packages];
    }

    public function file(string $lockPath, string $sidecarPath): void
    {
        $lock = self::read($lockPath);
        $sidecar = is_file($sidecarPath) ? self::read($sidecarPath) : [];
        $text = json_encode($this->packages($lock, $sidecar), JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES);
        if (file_put_contents($sidecarPath, $text . "\n") === false) {
            throw new RuntimeException("cannot write {$sidecarPath}");
        }
    }

    /**
     * @return array<string, mixed>
     */
    private static function read(string $path): array
    {
        $text = file_get_contents($path);
        if ($text === false) {
            throw new RuntimeException("cannot read {$path}");
        }
        return json_decode($text, true, 512, JSON_THROW_ON_ERROR);
    }
}
