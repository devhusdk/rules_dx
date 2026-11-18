<?php

declare(strict_types=1);

namespace Bazel\Runfiles;

/**
 * Finds the runfiles of a Bazel-built PHP program, from a runfiles directory or a manifest.
 */
final class Runfiles
{
    /** @var array<string, string> "source,apparent" to canonical repo name */
    private array $repoMapping = [];

    /** @var list<array{string, string, string}> source prefix, apparent and canonical name of `prefix*` rows */
    private array $prefixMapping = [];

    /**
     * @param array<string, string>|null $manifest runfiles key to real path, or null for a directory
     */
    private function __construct(
        private readonly ?string $directory,
        private readonly ?array $manifest,
        private readonly ?string $manifestFile,
    ) {
        $this->readRepoMapping();
    }

    /**
     * Opens the runfiles from RUNFILES_MANIFEST_FILE, RUNFILES_DIR or TEST_SRCDIR, else the ones beside the script.
     */
    public static function create(): self
    {
        $manifest = self::env('RUNFILES_MANIFEST_FILE');
        if ($manifest !== null && getenv('RUNFILES_MANIFEST_ONLY') === '1') {
            return self::fromManifest($manifest);
        }
        $directory = self::env('RUNFILES_DIR') ?? self::env('TEST_SRCDIR');
        if ($directory !== null && is_dir($directory)) {
            return self::fromDirectory($directory);
        }
        if ($manifest !== null) {
            return self::fromManifest($manifest);
        }
        $script = $_SERVER['SCRIPT_FILENAME'] ?? '';
        if (is_string($script) && $script !== '') {
            if (is_dir($script . '.runfiles')) {
                return self::fromDirectory($script . '.runfiles');
            }
            if (is_file($script . '.runfiles_manifest')) {
                return self::fromManifest($script . '.runfiles_manifest');
            }
        }
        throw new \RuntimeException('cannot find runfiles: set RUNFILES_DIR or RUNFILES_MANIFEST_FILE');
    }

    /**
     * Opens the runfiles directory at $directory.
     */
    public static function fromDirectory(string $directory): self
    {
        return new self(rtrim($directory, '/\\'), null, null);
    }

    /**
     * Opens the runfiles listed in the manifest file at $path.
     */
    public static function fromManifest(string $path): self
    {
        $lines = @file($path, FILE_IGNORE_NEW_LINES);
        if ($lines === false) {
            throw new \RuntimeException("cannot read runfiles manifest {$path}");
        }
        $entries = [];
        foreach ($lines as $line) {
            $line = rtrim($line, "\r");
            if ($line === '') {
                continue;
            }
            // A line that starts with a space escapes space, newline and backslash in both fields.
            if ($line[0] === ' ') {
                [$key, $target] = array_pad(explode(' ', substr($line, 1), 2), 2, '');
                $key = self::unescape($key, true);
                $target = self::unescape($target, false);
            } else {
                [$key, $target] = array_pad(explode(' ', $line, 2), 2, '');
            }
            $entries[$key] = $target;
        }
        return new self(null, $entries, $path);
    }

    /**
     * Returns the real path of runfiles key $path, or null when the manifest does not list it.
     *
     * $path is `repo/pkg/file`; an apparent repo name maps through the repo mapping of $sourceRepo,
     * the canonical name of the calling code's repo ("" for the main repo).
     */
    public function rlocation(string $path, string $sourceRepo = ''): ?string
    {
        if ($path === '') {
            throw new \InvalidArgumentException('runfiles path is empty');
        }
        if (self::isAbsolute($path)) {
            return $path;
        }
        foreach (explode('/', $path) as $segment) {
            if ($segment === '..' || $segment === '.') {
                throw new \InvalidArgumentException("runfiles path {$path} is not normalized");
            }
        }
        $parts = explode('/', $path, 2);
        $mapped = $this->mappedRepo($sourceRepo, $parts[0]);
        if ($mapped !== null) {
            $path = count($parts) === 2 ? $mapped . '/' . $parts[1] : $mapped;
        }
        return $this->lookup($path);
    }

    /**
     * Returns the environment a child Bazel program needs to find these runfiles.
     *
     * @return array<string, string>
     */
    public function envVars(): array
    {
        if ($this->manifestFile !== null) {
            return ['RUNFILES_MANIFEST_FILE' => $this->manifestFile];
        }
        return ['RUNFILES_DIR' => (string) $this->directory];
    }

    private function lookup(string $key): ?string
    {
        if ($this->manifest === null) {
            return $this->directory . '/' . $key;
        }
        if (array_key_exists($key, $this->manifest)) {
            return $this->manifest[$key] === '' ? null : $this->manifest[$key];
        }
        // A key below a listed directory resolves under that directory's real path.
        $prefix = $key;
        while (($cut = strrpos($prefix, '/')) !== false) {
            $prefix = substr($prefix, 0, $cut);
            $target = $this->manifest[$prefix] ?? '';
            if ($target !== '') {
                return $target . substr($key, $cut);
            }
        }
        return null;
    }

    private function mappedRepo(string $sourceRepo, string $apparent): ?string
    {
        $mapped = $this->repoMapping[$sourceRepo . ',' . $apparent] ?? null;
        if ($mapped !== null) {
            return $mapped;
        }
        foreach ($this->prefixMapping as [$prefix, $name, $target]) {
            if ($name === $apparent && str_starts_with($sourceRepo, $prefix)) {
                return $target;
            }
        }
        return null;
    }

    private function readRepoMapping(): void
    {
        $file = $this->lookup('_repo_mapping');
        if ($file === null || !is_file($file)) {
            return;
        }
        $lines = @file($file, FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES);
        if ($lines === false) {
            throw new \RuntimeException("cannot read repo mapping {$file}");
        }
        foreach ($lines as $line) {
            $fields = explode(',', rtrim($line, "\r"));
            if (count($fields) !== 3) {
                continue;
            }
            // Bazel writes one `prefix*` row for every repo of a module extension that shares a mapping.
            if (str_ends_with($fields[0], '*')) {
                $this->prefixMapping[] = [substr($fields[0], 0, -1), $fields[1], $fields[2]];
            } else {
                $this->repoMapping[$fields[0] . ',' . $fields[1]] = $fields[2];
            }
        }
    }

    private static function unescape(string $field, bool $key): string
    {
        $pairs = ['\\n' => "\n", '\\b' => '\\'];
        if ($key) {
            $pairs['\\s'] = ' ';
        }
        return strtr($field, $pairs);
    }

    private static function isAbsolute(string $path): bool
    {
        return $path[0] === '/' || $path[0] === '\\' || preg_match('/^[A-Za-z]:[\\\\\/]/', $path) === 1;
    }

    private static function env(string $name): ?string
    {
        $value = getenv($name);
        return $value === false || $value === '' ? null : $value;
    }
}
