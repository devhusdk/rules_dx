"""Public PHP API."""

load(
    "//php/private:defs.bzl",
    _PHP_COMPATIBLE_WITH = "PHP_COMPATIBLE_WITH",
    _php_bench = "php_bench",
    _php_binary = "php_binary",
    _php_library = "php_library",
    _php_test = "php_test",
)

PHP_COMPATIBLE_WITH = _PHP_COMPATIBLE_WITH
php_library = _php_library
php_binary = _php_binary
php_test = _php_test
php_bench = _php_bench
