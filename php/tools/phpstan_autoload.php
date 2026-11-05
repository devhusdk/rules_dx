<?php

declare(strict_types=1);

spl_autoload_register(static function (string $class): void {
    $roots = [
        'TomasVotruba\\UnusedPublic\\' => __DIR__ . '/unused-public/src/',
        'Webmozart\\Assert\\' => __DIR__ . '/assert/src/',
        'PHPStan\\Rules\\' => __DIR__ . '/../../vendor/phpstan/phpstan-strict-rules/src/Rules/',
    ];
    foreach ($roots as $prefix => $dir) {
        if (strncmp($class, $prefix, strlen($prefix)) === 0) {
            $file = $dir . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
            if (is_file($file)) {
                require $file;
            }
            return;
        }
    }
});
