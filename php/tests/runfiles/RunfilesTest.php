<?php

declare(strict_types=1);

namespace Bazel\Runfiles\Tests;

use Bazel\Runfiles\Runfiles;
use PHPUnit\Framework\TestCase;

final class RunfilesTest extends TestCase
{
    private string $dir;

    protected function setUp(): void
    {
        $base = getenv('TEST_TMPDIR') ?: sys_get_temp_dir();
        $this->dir = $base . '/' . uniqid('runfiles', true);
        mkdir($this->dir, 0777, true);
    }

    public function testDirectoryJoinsTheKey(): void
    {
        $runfiles = Runfiles::fromDirectory($this->dir . '/');
        $this->assertSame($this->dir . '/_main/a/b.txt', $runfiles->rlocation('_main/a/b.txt'));
        $this->assertSame(['RUNFILES_DIR' => $this->dir], $runfiles->envVars());
    }

    public function testManifestListsFilesAndDirectories(): void
    {
        $manifest = $this->write('m', "_main/a.txt /real/a.txt\n_main/tree /real/tree\n_main/empty \n");
        $runfiles = Runfiles::fromManifest($manifest);
        $this->assertSame('/real/a.txt', $runfiles->rlocation('_main/a.txt'));
        $this->assertSame('/real/tree/x/y.txt', $runfiles->rlocation('_main/tree/x/y.txt'));
        $this->assertNull($runfiles->rlocation('_main/empty'));
        $this->assertNull($runfiles->rlocation('_main/missing.txt'));
        $this->assertSame(['RUNFILES_MANIFEST_FILE' => $manifest], $runfiles->envVars());
    }

    public function testManifestUnescapesSpacesNewlinesAndBackslashes(): void
    {
        $manifest = $this->write('m', " _main/a\\sb\\nc\\bd.txt /real/a b\\nc\\bd.txt\n");
        $runfiles = Runfiles::fromManifest($manifest);
        $this->assertSame("/real/a b\nc\\d.txt", $runfiles->rlocation("_main/a b\nc\\d.txt"));
    }

    public function testRepoMappingResolvesApparentNames(): void
    {
        $mapping = $this->write('repo_mapping', ",my_dep,dep+\n,_main,_main\nrules_x+,dep,dep+\next+*,tool,tool+\n");
        $manifest = $this->write('m', "_repo_mapping {$mapping}\ndep+/f.txt /real/f.txt\ntool+/t /real/t\n");
        $runfiles = Runfiles::fromManifest($manifest);
        $this->assertSame('/real/f.txt', $runfiles->rlocation('my_dep/f.txt'));
        $this->assertSame('/real/f.txt', $runfiles->rlocation('dep/f.txt', 'rules_x+'));
        $this->assertSame('/real/t', $runfiles->rlocation('tool/t', 'ext+repo'));
        $this->assertNull($runfiles->rlocation('tool/t'));
    }

    public function testAbsolutePathsPassThrough(): void
    {
        $runfiles = Runfiles::fromDirectory($this->dir);
        $this->assertSame('/abs/x', $runfiles->rlocation('/abs/x'));
        $this->assertSame('C:/abs/x', $runfiles->rlocation('C:/abs/x'));
    }

    public function testRejectsUnnormalizedKeys(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        Runfiles::fromDirectory($this->dir)->rlocation('_main/../x');
    }

    public function testMissingManifestThrows(): void
    {
        $this->expectException(\RuntimeException::class);
        Runfiles::fromManifest($this->dir . '/none');
    }

    private function write(string $name, string $text): string
    {
        $path = $this->dir . '/' . $name;
        file_put_contents($path, $text);
        return $path;
    }
}
