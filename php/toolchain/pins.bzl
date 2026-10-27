"""Pinned PHP runtime, Composer and PHPUnit identities."""

PHP_VERSION = "8.5.12"

# dl.static-php.dev rebuilds files in place; list an immutable mirror first.
STATIC_PHP = "https://dl.static-php.dev/v3/php-bin/common/"

PHP_CELLS = {
    "linux_amd64": struct(
        archive = "php-" + PHP_VERSION + "-cli-linux-x86_64.tar.gz",
        sha256 = "42a262e80e0accfa8bab30777c56519f9764d10128fa7ce38777c549de176225",
        urls = [STATIC_PHP + "php-" + PHP_VERSION + "-cli-linux-x86_64.tar.gz"],
    ),
}

COMPOSER = struct(
    version = "2.10.3",
    sha256 = "7a2d379d5b8ffdaa028580ef26494c36d2feef4b178d3dd1473a4dbc5e17c8d6",
    urls = ["https://getcomposer.org/download/2.10.3/composer.phar"],
)

PHPUNIT = struct(
    version = "13.4.3",
    sha256 = "f9f8f986f2b6e862187a04619eb01b5960cea77bb39bd1e38d3e396a99eae516",
    urls = ["https://phar.phpunit.de/phpunit-13.4.3.phar"],
)
