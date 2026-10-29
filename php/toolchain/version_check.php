<?php

declare(strict_types=1);

// Usage: version_check.php <version> <extension>...
[, $want] = $argv + [null, null];
if (PHP_VERSION !== $want) {
    fwrite(STDERR, 'resolved PHP is ' . PHP_VERSION . ', the pin is ' . $want . "\n");
    exit(1);
}
foreach (array_slice($argv, 2) as $module) {
    if (!extension_loaded($module)) {
        fwrite(STDERR, 'resolved PHP lacks the ' . $module . " extension\n");
        exit(1);
    }
}
