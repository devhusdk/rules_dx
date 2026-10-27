<?php

declare(strict_types=1);

use Hello\Greeter;

$path = getenv('RUNFILES_DIR') . '/' . ($argv[1] ?? '');
$name = file_get_contents($path);
if ($name === false) {
    fwrite(STDERR, "cannot read {$path}\n");
    exit(2);
}
echo (new Greeter())->greet($name), "\n";
echo 'precision=', ini_get('precision'), "\n";
