<?php

declare(strict_types=1);

function fail(string $message): never
{
    fwrite(STDERR, $message . "\n");
    exit(1);
}

function run(array $command, array $env): array
{
    $pipes = [];
    $process = proc_open($command, [0 => ['file', '/dev/null', 'r'], 1 => ['pipe', 'w'], 2 => ['redirect', 1]], $pipes, null, $env);
    if ($process === false) {
        fail('cannot start ' . $command[0]);
    }
    $out = stream_get_contents($pipes[1]);
    fclose($pipes[1]);
    return [proc_close($process), $out];
}

function lint(array $args): void
{
    [$marker, $src] = $args + [null, null];
    if ($marker === null || $src === null) {
        fail('usage: tool.php lint <marker> <src>');
    }
    [$code, $output] = run([PHP_BINARY, '-n', '-d', 'display_errors=stderr', '-l', $src], []);
    if ($code !== 0) {
        fail(trim($output));
    }
    file_put_contents($marker, "ok\n");
}

function mkdirs(string $dir): void
{
    if (!is_dir($dir) && !mkdir($dir, 0o755, true) && !is_dir($dir)) {
        fail('cannot create ' . $dir);
    }
}

function removeTree(string $dir): void
{
    if (!is_dir($dir)) {
        return;
    }
    foreach (scandir($dir) as $entry) {
        if ($entry === '.' || $entry === '..') {
            continue;
        }
        $path = $dir . '/' . $entry;
        is_dir($path) && !is_link($path) ? removeTree($path) : unlink($path);
    }
    rmdir($dir);
}

const PACKAGE_SKIP = ['BUILD.bazel', 'REPO.bazel', 'MODULE.bazel', 'WORKSPACE', 'WORKSPACE.bazel', 'manifest.json'];

/** @return array<string, string> relative path => source path, sorted, of the files below a package root. */
function packageFiles(string $root): array
{
    $found = [];
    $walk = function (string $dir, string $rel) use (&$walk, &$found, $root): void {
        $entries = scandir($dir);
        if ($entries === false) {
            fail('cannot read ' . $dir);
        }
        foreach ($entries as $entry) {
            if ($entry === '.' || $entry === '..' || ($rel === '' && in_array($entry, PACKAGE_SKIP, true))) {
                continue;
            }
            $path = $dir . '/' . $entry;
            $name = $rel === '' ? $entry : $rel . '/' . $entry;
            if (is_dir($path)) {
                $walk($path, $name);
            } elseif (is_file($path)) {
                $found[$name] = $path;
            }
        }
    };
    $walk($root, '');
    ksort($found, SORT_STRING);
    return $found;
}

/**
 * Checks the ext-* requirements of the staged packages. The check runs on the exec php; that is valid
 * while exec and target use the same static PHP build.
 */
function checkExtensions(array $manifests): void
{
    $loaded = array_map('strtolower', get_loaded_extensions());
    foreach ($manifests as $name => $manifest) {
        foreach (array_keys($manifest['require'] ?? []) as $requirement) {
            if (str_starts_with($requirement, 'ext-') && !in_array(strtolower(substr($requirement, 4)), $loaded, true)) {
                fail($name . ' requires the PHP extension ' . substr($requirement, 4) . ', which the pinned PHP does not provide');
            }
        }
    }
}

function autoload(array $args): void
{
    if (count($args) < 3) {
        fail('usage: tool.php autoload <composer.phar> <out dir> <suffix> [--src <path> <staged>]... [--file <staged>]... [--package <manifest> <root>]...');
    }
    [$composer, $out, $suffix] = array_splice($args, 0, 3);
    $staged = [];
    $files = [];
    $packages = [];
    while ($args !== []) {
        $flag = array_shift($args);
        if ($flag === '--src' && count($args) >= 2) {
            [$path, $target] = array_splice($args, 0, 2);
            $staged[$target] = $path;
        } elseif ($flag === '--file' && $args !== []) {
            $files[] = array_shift($args);
        } elseif ($flag === '--package' && count($args) >= 2) {
            [$manifestPath, $root] = array_splice($args, 0, 2);
            $manifest = json_decode((string) file_get_contents($manifestPath), true);
            if (!is_array($manifest) || !isset($manifest['name'])) {
                fail('bad package manifest ' . $manifestPath);
            }
            if (isset($packages[$manifest['name']]) && $packages[$manifest['name']][1] !== $root) {
                fail('package ' . $manifest['name'] . ' comes from two repositories: ' . $packages[$manifest['name']][1] . ' and ' . $root);
            }
            $packages[$manifest['name']] = [$manifest, $root];
        } else {
            fail('unexpected argument ' . $flag);
        }
    }
    ksort($staged, SORT_STRING);
    ksort($packages, SORT_STRING);
    checkExtensions(array_map(static fn(array $package): array => $package[0], $packages));
    mkdirs($out);
    $installed = [];
    foreach ($packages as $name => [$manifest, $root]) {
        foreach (packageFiles($root) as $rel => $path) {
            mkdirs(dirname($out . '/vendor/' . $name . '/' . $rel));
            if (!copy($path, $out . '/vendor/' . $name . '/' . $rel)) {
                fail('cannot stage ' . $path);
            }
        }
        $manifest['install-path'] = '../' . $name;
        $installed[] = $manifest;
    }
    foreach ($staged as $target => $path) {
        mkdirs(dirname($out . '/' . $target));
        if (!copy($path, $out . '/' . $target)) {
            fail('cannot stage ' . $path);
        }
    }
    $manifest = [
        'name' => 'dx/autoload',
        'version' => '0.0.0',
        'autoload' => ['classmap' => array_keys($staged)],
        'config' => [
            'autoloader-suffix' => $suffix,
            'platform-check' => false,
        ],
    ];
    if ($files !== []) {
        $manifest['autoload']['files'] = array_values(array_unique($files));
    }
    $flags = JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR;
    file_put_contents($out . '/composer.json', json_encode($manifest, $flags) . "\n");
    mkdirs($out . '/vendor/composer');
    file_put_contents($out . '/vendor/composer/installed.json', json_encode(['packages' => $installed, 'dev' => true, 'dev-package-names' => []], $flags) . "\n");
    $home = sys_get_temp_dir() . '/dx-composer-' . bin2hex(random_bytes(8));
    mkdirs($home . '/cache');
    $env = [
        'COMPOSER_ALLOW_SUPERUSER' => '1',
        'COMPOSER_CACHE_DIR' => $home . '/cache',
        'COMPOSER_DISABLE_NETWORK' => '1',
        'COMPOSER_HOME' => $home,
        'COMPOSER_NO_AUDIT' => '1',
        'COMPOSER_NO_INTERACTION' => '1',
        'COMPOSER_ROOT_VERSION' => '0.0.0',
        'HOME' => $home,
        'PATH' => '/nonexistent',
    ];
    [$code, $output] = run([PHP_BINARY, '-n', $composer, 'dump-autoload', '--classmap-authoritative', '--no-scripts', '--no-plugins', '--no-ansi', '--working-dir=' . $out], $env);
    removeTree($home);
    if ($code !== 0) {
        fail("composer dump-autoload failed:\n" . trim($output));
    }
}

$command = $argv[1] ?? '';
$rest = array_slice($argv, 2);
match ($command) {
    'lint' => lint($rest),
    'autoload' => autoload($rest),
    default => fail('usage: tool.php lint|autoload ...'),
};
