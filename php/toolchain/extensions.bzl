"""Module extension for php/toolchain."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "COMPOSER", "PCOV", "PCRE2_HEADERS", "PHPBENCH", "PHPCS", "PHPDOCUMENTOR", "PHPSTAN", "PHPSTAN_STRICT_RULES", "PHPUNIT", "PHP_CELLS", "PHP_HEADERS", "PHP_VERSION", "UNUSED_PUBLIC", "WEBMOZART_ASSERT")
load(":repos.bzl", "HEADERS_BUILD", "HUB_NAME", "RUNTIME_PREFIX", "SOURCES_BUILD", "VERSION_FILE", "hub_build", "phar_build", "runtime_build")


_PHP_INCLUDES = ["include/php", "include/php/main", "include/php/Zend", "include/php/TSRM", "include/php/ext"]


def _php_impl(mctx):
    for cell in sorted(PHP_CELLS.keys()):
        pinned_archive(
            name = RUNTIME_PREFIX + cell,
            files = {
                "BUILD.bazel": runtime_build(PHP_VERSION, PHP_CELLS[cell].interpreter, PHP_CELLS[cell].pcov),
                VERSION_FILE: PHP_VERSION + "\n",
            },
            required = [PHP_CELLS[cell].interpreter],
            sha256 = PHP_CELLS[cell].sha256,
            urls = PHP_CELLS[cell].urls,
        )
    for cell, headers in sorted(PHP_HEADERS.items()):
        pinned_archive(
            name = "dx_php_headers_" + cell,
            files = {"BUILD.bazel": HEADERS_BUILD.format(includes = repr(_PHP_INCLUDES))},
            inner = [headers.inner],
            required = ["include/php/main/php_config.h"],
            sha256 = headers.sha256,
            type = "zip",
            urls = headers.urls,
        )
    pinned_archive(
        name = "dx_php_pcre2",
        files = {"BUILD.bazel": HEADERS_BUILD.format(includes = repr(["include"]))},
        inner = [PCRE2_HEADERS.inner],
        required = ["include/pcre2.h"],
        sha256 = PCRE2_HEADERS.sha256,
        type = "zip",
        urls = PCRE2_HEADERS.urls,
    )
    pinned_archive(
        name = "dx_php_pcov",
        files = {"BUILD.bazel": mctx.read(Label("//php/toolchain:pcov.BUILD"))},
        required = ["pcov-" + PCOV.version + "/pcov.c"],
        sha256 = PCOV.sha256,
        urls = PCOV.urls,
    )
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(PHP_CELLS.keys())})
    for name, file, pin in [
        ("dx_composer", "composer.phar", COMPOSER),
        ("dx_phpunit", "phpunit.phar", PHPUNIT),
        ("dx_phpbench", "phpbench.phar", PHPBENCH),
        ("dx_phpcs", "phpcs.phar", PHPCS),
        ("dx_phpdocumentor", "phpDocumentor.phar", PHPDOCUMENTOR),
        ("dx_phpstan", "phpstan.phar", PHPSTAN),
    ]:
        pinned_archive(name = name, download_as = file, files = {"BUILD.bazel": phar_build(file)}, sha256 = pin.sha256, urls = pin.urls)
    for name, pin in [
        ("dx_unused_public", UNUSED_PUBLIC),
        ("dx_webmozart_assert", WEBMOZART_ASSERT),
        ("dx_phpstan_strict_rules", PHPSTAN_STRICT_RULES),
    ]:
        pinned_archive(
            name = name,
            files = {"BUILD.bazel": SOURCES_BUILD},
            required = ["src"],
            sha256 = pin.sha256,
            strip_prefix = pin.strip_prefix,
            urls = pin.urls,
        )
    return mctx.extension_metadata(reproducible = True)


php = module_extension(
    implementation = _php_impl,
    doc = "Creates the pinned PHP runtimes, Composer, PHPUnit and the toolchain hub.",
)
