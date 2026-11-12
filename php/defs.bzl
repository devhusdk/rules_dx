"""Public PHP API."""

load(
    "//php/private:defs.bzl",
    _php_bench = "php_bench",
    _php_binary = "php_binary",
    _php_library = "php_library",
    _php_test = "php_test",
)

php_library = _php_library
php_binary = _php_binary
php_test = _php_test
php_bench = _php_bench
