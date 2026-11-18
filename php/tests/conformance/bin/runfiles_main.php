<?php

declare(strict_types=1);

use Bazel\Runfiles\Runfiles;

$path = Runfiles::create()->rlocation($argv[1] ?? '');
$text = $path === null ? false : file_get_contents($path);
if ($text === false) {
    fwrite(STDERR, 'cannot read runfile ' . ($argv[1] ?? '') . "\n");
    exit(2);
}
echo $text;
