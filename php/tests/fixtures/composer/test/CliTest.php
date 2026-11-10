<?php

declare(strict_types=1);

use PHPUnit\Framework\TestCase;

final class CliTest extends TestCase
{
    public function testTheMainBinaryLoadsTheRequireEdge(): void
    {
        [$code, $out] = $this->execute('MAIN_BIN');
        $this->assertSame(0, $code, $out);
        $this->assertSame('RequestFactoryInterface=loaded deprecation=files', rtrim($out, "\r\n"));
    }

    public function testAClassOutsideDepsDoesNotLoad(): void
    {
        [$code, $out] = $this->execute('MISSING_BIN');
        $this->assertNotSame(0, $code, 'a class of a package outside deps must not load: ' . $out);
        $this->assertStringContainsString('Psr\Http\Message\RequestInterface', $out);
    }

    public function testTheAutoloadTreeIsReproducible(): void
    {
        $this->assertSame($this->files((string) getenv('TREE_A')), $this->files((string) getenv('TREE_B')));
    }

    /** @return array{int, string} */
    private function execute(string $variable): array
    {
        $binary = getenv($variable);
        $this->assertIsString($binary, $variable . ' is not set');
        $process = proc_open([$binary], [1 => ['pipe', 'w'], 2 => ['redirect', 1]], $pipes);
        $this->assertIsResource($process);
        $out = (string) stream_get_contents($pipes[1]);
        fclose($pipes[1]);

        return [proc_close($process), $out];
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
