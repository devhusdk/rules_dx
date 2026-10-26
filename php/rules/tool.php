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
    // stderr shares the stdout pipe, so a chatty child cannot fill one pipe while we block reading the other.
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

function autoload(array $args): void
{
    if (count($args) < 3) {
        fail('usage: tool.php autoload <composer.phar> <out dir> <suffix> [--src <path> <staged>]... [--file <staged>]...');
    }
    [$composer, $out, $suffix] = array_splice($args, 0, 3);
    $staged = [];
    $files = [];
    while ($args !== []) {
        $flag = array_shift($args);
        if ($flag === '--src' && count($args) >= 2) {
            [$path, $target] = array_splice($args, 0, 2);
            $staged[$target] = $path;
        } elseif ($flag === '--file' && $args !== []) {
            $files[] = array_shift($args);
        } else {
            fail('unexpected argument ' . $flag);
        }
    }
    ksort($staged, SORT_STRING);
    mkdirs($out);
    foreach ($staged as $target => $path) {
        mkdirs(dirname($out . '/' . $target));
        if (!copy($path, $out . '/' . $target)) {
            fail('cannot stage ' . $path);
        }
    }
    $manifest = [
        // A fixed name: the suffix may hold runs of '_' that Composer's package-name schema rejects.
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
    file_put_contents($out . '/vendor/composer/installed.json', json_encode(['packages' => [], 'dev' => true, 'dev-package-names' => []], $flags) . "\n");
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
