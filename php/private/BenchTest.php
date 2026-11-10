<?php

declare(strict_types=1);

namespace Dx\Bench;

use PHPUnit\Framework\TestCase;

final class BenchTest extends TestCase
{
    public function testRunsPhpBench(): void
    {
        $srcdir = (string) getenv('TEST_SRCDIR');
        $phar = $srcdir . '/' . (string) getenv('DX_PHPBENCH');
        $files = [];
        foreach (explode(' ', (string) getenv('DX_BENCH_FILES')) as $file) {
            $files[] = $srcdir . '/' . $file;
        }
        $out = (string) getenv('TEST_UNDECLARED_OUTPUTS_DIR');
        $dump = $out . '/phpbench.xml';

        $command = array_merge(
            [PHP_BINARY, $phar, 'run'],
            $files,
            ['--progress=none', '--dump-file=' . $dump],
        );
        exec(implode(' ', array_map('escapeshellarg', $command)) . ' 2>&1', $lines, $status);
        self::assertSame(0, $status, implode("\n", $lines));

        $suite = simplexml_load_file($dump);
        self::assertNotFalse($suite);
        $results = [];
        foreach ($suite->xpath('//subject') ?: [] as $subject) {
            foreach ($subject->variant as $variant) {
                $revs = (int) $variant['revs'];
                $times = [];
                foreach ($variant->iteration as $iteration) {
                    $times[] = (float) $iteration['time-net'] / $revs / 1e6;
                }
                if ($times === []) {
                    continue;
                }
                $mean = array_sum($times) / count($times);
                $variance = 0.0;
                foreach ($times as $time) {
                    $variance += ($time - $mean) ** 2;
                }
                $results[] = [
                    'name' => (string) $subject['name'],
                    'unit' => 's',
                    'mean' => $mean,
                    'stddev' => sqrt($variance / count($times)),
                    'iterations' => count($times) * $revs,
                ];
            }
        }
        self::assertNotSame([], $results, 'phpbench reported no results');
        file_put_contents($out . '/dx-bench.json', json_encode(['schema' => 'dx-bench/v1', 'results' => $results]));
    }
}
