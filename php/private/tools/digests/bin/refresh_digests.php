<?php

declare(strict_types=1);

use Digests\Refresh;

if (count($argv) !== 3) {
    fwrite(STDERR, "usage: refresh_digests <composer.lock> <composer.digests.json>\n");
    exit(2);
}

$fetch = static function (string $url): string {
    $bytes = file_get_contents($url);
    if ($bytes === false) {
        throw new RuntimeException("cannot download {$url}");
    }
    return $bytes;
};

try {
    (new Refresh($fetch))->file($argv[1], $argv[2]);
} catch (Throwable $error) {
    fwrite(STDERR, $error->getMessage() . "\n");
    exit(1);
}
