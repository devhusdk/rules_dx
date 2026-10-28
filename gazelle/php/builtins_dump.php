<?php
// Prints the sorted, lowercased names of the classes, interfaces, traits,
// functions and constants the running PHP provides without user code, one per
// line. With --check <file>, diffs that list against <file> instead and exits
// non-zero on drift.
declare(strict_types=1);

function builtin_names(): array
{
    $names = [];
    foreach ([get_declared_classes(), get_declared_interfaces(), get_declared_traits()] as $group) {
        foreach ($group as $class) {
            if ((new ReflectionClass($class))->isInternal()) {
                $names[strtolower($class)] = true;
            }
        }
    }
    foreach (get_defined_functions()['internal'] as $fn) {
        $names[strtolower($fn)] = true;
    }
    foreach (get_defined_constants(true) as $category => $constants) {
        if ($category === 'user') {
            continue;
        }
        foreach (array_keys($constants) as $constant) {
            $names[strtolower((string) $constant)] = true;
        }
    }
    $names = array_keys($names);
    sort($names, SORT_STRING);
    return $names;
}

$want = builtin_names();
if (count($argv) > 2 && $argv[1] === '--check') {
    $lines = file($argv[2], FILE_IGNORE_NEW_LINES | FILE_SKIP_EMPTY_LINES);
    if ($lines === false) {
        fwrite(STDERR, "read {$argv[2]} failed\n");
        exit(1);
    }
    $missing = array_diff($want, $lines);
    $extra = array_diff($lines, $want);
    if (!$missing && !$extra) {
        exit(0);
    }
    fwrite(STDERR, 'builtins.txt drifted from PHP ' . PHP_VERSION . "; regenerate with: bazel run //gazelle/php:builtins_dump > gazelle/php/builtins.txt\n");
    foreach ($missing as $name) {
        fwrite(STDERR, "missing: $name\n");
    }
    foreach ($extra as $name) {
        fwrite(STDERR, "extra: $name\n");
    }
    exit(1);
}
foreach ($want as $name) {
    echo $name, "\n";
}
