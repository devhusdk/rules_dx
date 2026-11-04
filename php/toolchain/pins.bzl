"""Pinned PHP runtime, Composer and PHPUnit identities."""

PHP_VERSION = "8.5.12"

# dl.static-php.dev rebuilds files in place, so a pin can go stale.
STATIC_PHP = "https://dl.static-php.dev/v3/php-bin/"

def _cell(arch, os_name, cpu, sha256, flavor = "common", ext = "tar.gz", interpreter = "php"):
    archive = "php-" + PHP_VERSION + "-cli-" + arch + "." + ext
    return struct(
        archive = archive,
        exec_compatible_with = ["@platforms//os:" + os_name, "@platforms//cpu:" + cpu],
        interpreter = interpreter,
        sha256 = sha256,
        urls = [STATIC_PHP + flavor + "/" + archive],
    )

PHP_CELLS = {
    "linux_amd64": _cell("linux-x86_64", "linux", "x86_64", "3e73114b61328dd7ba86a2ebe2050855dffa207d9d8b35ca76d763a6fc5e1807"),
    "linux_arm64": _cell("linux-aarch64", "linux", "aarch64", "19f38cbeb05156d6e1a7401800f0394f78204ddf1f873a87b595c7cb1d308f64"),
    "macos_arm64": _cell("macos-aarch64", "macos", "aarch64", "3d07ac95616814ed1a97ab47b4ccf089fdd522bf2fc500e00f59fef5749c44df"),
    "windows_x86_64": _cell("win", "windows", "x86_64", "722b336feb6187b0e42c4e499a4731c877aa6a6f555fb886aeeceb30cf84f326", flavor = "spc-max", ext = "zip", interpreter = "php.exe"),
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
