"""Pinned PHP runtime, Composer and PHPUnit identities."""

PHP_VERSION = "8.5.12"

# dl.static-php.dev rebuilds files in place, so a pin can go stale.
STATIC_PHP = "https://dl.static-php.dev/v3/php-bin/"

def _cell(arch, os_name, cpu, sha256, flavor = "common", ext = "tar.gz", interpreter = "php", pcov = False):
    archive = "php-" + PHP_VERSION + "-cli-" + arch + "." + ext
    return struct(
        archive = archive,
        exec_compatible_with = ["@platforms//os:" + os_name, "@platforms//cpu:" + cpu],
        interpreter = interpreter,
        pcov = pcov,
        sha256 = sha256,
        urls = [STATIC_PHP + flavor + "/" + archive],
    )

PHP_CELLS = {
    "linux_amd64": _cell("linux-x86_64", "linux", "x86_64", "6ccf92eed2d6c0d1820d51cd0bae3134b8d7c3d66f0a5532505525f8b3350acb", flavor = "gnu-bulk", pcov = True),
    "linux_arm64": _cell("linux-aarch64", "linux", "aarch64", "bee9b8f9ed82fb9c4c2d48eaf099c500d5463dd70d4decbe033dc3aedc7955fc", flavor = "gnu-bulk", pcov = True),
    "macos_arm64": _cell("macos-aarch64", "macos", "aarch64", "3d07ac95616814ed1a97ab47b4ccf089fdd522bf2fc500e00f59fef5749c44df"),
    "windows_x86_64": _cell("win", "windows", "x86_64", "722b336feb6187b0e42c4e499a4731c877aa6a6f555fb886aeeceb30cf84f326", flavor = "spc-max", ext = "zip", interpreter = "php.exe"),
}

# The gnu-bulk Linux builds are glibc and can load extensions; the common ones are static musl.
# PCOV is built from source against the conda-forge php headers (NTS, same API as the pinned php).
PCOV = struct(
    version = "1.0.12",
    sha256 = "23255c8c9335a9636ccb743f5302436a97a582a0bbde9869485be911bbc15da8",
    urls = ["https://pecl.php.net/get/pcov-1.0.12.tgz"],
)

CONDA_FORGE = "https://conda.anaconda.org/conda-forge/"

PHP_HEADERS = {
    "linux_amd64": struct(
        inner = "pkg-php-8.5.12-h2a25e86_0.tar.zst",
        sha256 = "2a1e0b77c218dcc68f054a5b7a3c7fe3593fb7ad91e9ae9d98d63251753edbd1",
        urls = [CONDA_FORGE + "linux-64/php-8.5.12-h2a25e86_0.conda"],
    ),
    "linux_arm64": struct(
        inner = "pkg-php-8.5.12-h36107db_0.tar.zst",
        sha256 = "b022c23890ed69dbf091ae5f16b07de3f21a0a04779d6f86a925a9e343098929",
        urls = [CONDA_FORGE + "linux-aarch64/php-8.5.12-h36107db_0.conda"],
    ),
}

PCRE2_HEADERS = struct(
    inner = "pkg-pcre2-10.47-h8b3dc9c_1.tar.zst",
    sha256 = "9ebb10a4eb3be51ae67d85c679f5a7a50cb040ed25c783e6331a225ea09991d4",
    urls = [CONDA_FORGE + "linux-64/pcre2-10.47-h8b3dc9c_1.conda"],
)

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

UNUSED_PUBLIC = struct(
    version = "2.2.1",
    sha256 = "12b24273cc1745c2ab8ba1f49dbb7f5984296bb41fd1fac0de8cb30cfde72543",
    strip_prefix = "unused-public-2.2.1",
    urls = ["https://github.com/TomasVotruba/unused-public/archive/refs/tags/2.2.1.tar.gz"],
)

WEBMOZART_ASSERT = struct(
    version = "2.5.0",
    sha256 = "4dc1ffd9e4060770b1a8dd582427ce2394bd5164557cf07ce55928e00eeab184",
    strip_prefix = "assert-2.5.0",
    urls = ["https://github.com/webmozart/assert/archive/refs/tags/2.5.0.tar.gz"],
)

PHPSTAN = struct(
    version = "2.3.3",
    sha256 = "aa5cc7543f0de0ae7997deb24f654a3b2454b6ba4ba2c4552957a175886b60b1",
    urls = ["https://github.com/phpstan/phpstan/releases/download/2.3.3/phpstan.phar"],
)
