<?php

declare(strict_types=1);

namespace Hello\Tests;

use PHPUnit\Framework\TestCase;

final class CliTest extends TestCase
{
    public function testHelloPrintsTheGreetingAndThePrecision(): void
    {
        $process = proc_open([$this->env('HELLO_BIN'), $this->env('NAME_RLOCATION')], [1 => ['pipe', 'w'], 2 => ['redirect', 1]], $pipes);
        $this->assertIsResource($process);
        $out = stream_get_contents($pipes[1]);
        fclose($pipes[1]);
        $this->assertSame(0, proc_close($process), (string) $out);
        $this->assertSame("Hello, WORLD!\nprecision=7", rtrim(str_replace("\r\n", "\n", (string) $out), "\n"));
    }

    public function testTheAutoloadTreeIsReproducible(): void
    {
        $this->assertSame($this->files($this->env('TREE_A')), $this->files($this->env('TREE_B')));
    }

    private function env(string $name): string
    {
        $value = getenv($name);
        $this->assertIsString($value, $name . ' is not set');

        return $value;
    }

    /** @return array<string, string> relative path => content hash */
    private function files(string $root): array
    {
        $found = [];
        $iterator = new \RecursiveIteratorIterator(new \RecursiveDirectoryIterator($root, \FilesystemIterator::SKIP_DOTS));
        foreach ($iterator as $file) {
            if ($file->isFile()) {
                $found[substr($file->getPathname(), strlen($root) + 1)] = hash_file('sha256', $file->getPathname());
            }
        }
        ksort($found);
        $this->assertNotSame([], $found);

        return $found;
    }
}
