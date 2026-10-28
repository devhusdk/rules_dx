"""Pinned PHP runtime, Composer and PHPUnit identities."""

PHP_VERSION = "8.5.12"

# dl.static-php.dev rebuilds files in place, so a pin can go stale; an immutable
# mirror listed first is tracked as a follow-up to #4552.
STATIC_PHP = "https://dl.static-php.dev/v3/php-bin/common/"

def _cell(arch, os_name, cpu, sha256):
    archive = "php-" + PHP_VERSION + "-cli-" + arch + ".tar.gz"
    return struct(
        archive = archive,
        exec_compatible_with = ["@platforms//os:" + os_name, "@platforms//cpu:" + cpu],
        sha256 = sha256,
        urls = [STATIC_PHP + archive],
    )

PHP_CELLS = {
    "linux_amd64": _cell("linux-x86_64", "linux", "x86_64", "a626899b3177acd97e4e16ce60200935f31c51c9fcbd118d7719f9cbb3781ab6"),
    "linux_arm64": _cell("linux-aarch64", "linux", "aarch64", "b66621bfbd39146d9532707163fcc6f5bd537f36284508faa62e8d03d812496e"),
    "macos_arm64": _cell("macos-aarch64", "macos", "aarch64", "9aa0431f040199a69f99e37378ee0134e81589cb901ca8a942127115463120f8"),
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

PHPCS = struct(
    version = "4.0.4",
    sha256 = "4b010cd21d8bc8a17e2504792e3c77ef8259126f24caf7eafe7eddef1fa871e9",
    urls = ["https://github.com/PHPCSStandards/PHP_CodeSniffer/releases/download/4.0.4/phpcs.phar"],
)

PHPSTAN = struct(
    version = "2.3.3",
    sha256 = "aa5cc7543f0de0ae7997deb24f654a3b2454b6ba4ba2c4552957a175886b60b1",
    urls = ["https://github.com/phpstan/phpstan/releases/download/2.3.3/phpstan.phar"],
)
