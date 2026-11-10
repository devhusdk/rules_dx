"""Module extension for php/private/toolchain."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//core:repo.bzl", "hub_repo", "pinned_archive")
load(":cells.bzl", "PHP_CELLS")
load(":repos.bzl", "HEADERS_BUILD", "HUB_NAME", "RUNTIME_PREFIX", "SOURCES_BUILD", "VERSION_FILE", "hub_build", "phar_build", "runtime_build")


_PHP_INCLUDES = ["include/php", "include/php/main", "include/php/Zend", "include/php/TSRM", "include/php/ext"]


def _php_impl(mctx):
    pins = PINS["dx_php"]
    php_version = pins["version"]
    pcov = pins["pcov"]
    pcre2 = pins["pcre2"]
    tools = pins["tools"]
    for cell in sorted(PHP_CELLS.keys()):
        pinned_archive(
            name = RUNTIME_PREFIX + cell,
            files = {
                "BUILD.bazel": runtime_build(php_version, PHP_CELLS[cell].interpreter, PHP_CELLS[cell].pcov),
                VERSION_FILE: php_version + "\n",
            },
            required = [PHP_CELLS[cell].interpreter],
            sha256 = pins["cells"][cell]["sha256"],
            urls = pins["cells"][cell]["urls"],
        )
    for cell, headers in sorted(pins["headers"].items()):
        pinned_archive(
            name = "dx_php_headers_" + cell,
            files = {"BUILD.bazel": HEADERS_BUILD.format(includes = repr(_PHP_INCLUDES))},
            inner = [headers["inner"]],
            required = ["include/php/main/php_config.h"],
            sha256 = headers["sha256"],
            type = "zip",
            urls = headers["urls"],
        )
    pinned_archive(
        name = "dx_php_pcre2",
        files = {"BUILD.bazel": HEADERS_BUILD.format(includes = repr(["include"]))},
        inner = [pcre2["inner"]],
        required = ["include/pcre2.h"],
        sha256 = pcre2["sha256"],
        type = "zip",
        urls = pcre2["urls"],
    )
    pinned_archive(
        name = "dx_php_pcov",
        files = {"BUILD.bazel": mctx.read(Label("//php/private/toolchain:pcov.BUILD"))},
        required = ["pcov-" + pcov["version"] + "/pcov.c"],
        sha256 = pcov["sha256"],
        urls = pcov["urls"],
    )
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(PHP_CELLS.keys())})
    for name, file, pin in [
        ("dx_composer", "composer.phar", tools["composer"]),
        ("dx_phpunit", "phpunit.phar", tools["phpunit"]),
        ("dx_phpbench", "phpbench.phar", tools["phpbench"]),
        ("dx_phpcs", "phpcs.phar", tools["phpcs"]),
        ("dx_phpdocumentor", "phpDocumentor.phar", tools["phpdocumentor"]),
        ("dx_phpstan", "phpstan.phar", tools["phpstan"]),
    ]:
        pinned_archive(name = name, download_as = file, files = {"BUILD.bazel": phar_build(file)}, sha256 = pin["sha256"], urls = pin["urls"])
    for name, pin in [
        ("dx_unused_public", tools["unused_public"]),
        ("dx_webmozart_assert", tools["webmozart_assert"]),
        ("dx_phpstan_strict_rules", tools["phpstan_strict_rules"]),
    ]:
        pinned_archive(
            name = name,
            files = {"BUILD.bazel": SOURCES_BUILD},
            required = ["src"],
            sha256 = pin["sha256"],
            strip_prefix = pin["strip_prefix"],
            urls = pin["urls"],
        )
    return mctx.extension_metadata(reproducible = True)


php = module_extension(
    implementation = _php_impl,
    doc = "Creates the pinned PHP runtimes, Composer, PHPUnit and the toolchain hub.",
)
