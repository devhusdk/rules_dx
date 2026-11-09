"""Hermetic PHP, Composer and PHPUnit acquisition and toolchain registration."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load("//libs/starlark:toolchain.bzl", "toolchain_target")
load(":pins.bzl", "COMPOSER", "PCOV", "PCRE2_HEADERS", "PHPBENCH", "PHPCS", "PHPDOCUMENTOR", "PHPSTAN", "PHPSTAN_STRICT_RULES", "PHPUNIT", "PHP_CELLS", "PHP_HEADERS", "PHP_VERSION", "UNUSED_PUBLIC", "WEBMOZART_ASSERT")

HUB_NAME = "dx_php_toolchains"

RUNTIME_PREFIX = "dx_php_"

VERSION_FILE = "DX_PHP_VERSION"

TOOLCHAIN_TYPE = str(Label("//php/toolchain:toolchain_type"))

EXEC_TOOLCHAIN_TYPE = str(Label("//php/toolchain:exec_toolchain_type"))

_RUNTIME_BUILD = """load("{php_bzl}", "php_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["{interpreter}", "{version_file}"])

php_toolchain(
    name = "php_toolchain",
    interpreter = "{interpreter}",
    pcov = {pcov},
    version = "{version}",
)
"""

PCOV_LABEL = "@dx_php_pcov//:pcov.so"

_HEADERS_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

cc_library(
    name = "headers",
    hdrs = glob(["include/**/*.h"]),
    includes = {includes},
)
"""

_PHP_INCLUDES = ["include/php", "include/php/main", "include/php/Zend", "include/php/TSRM", "include/php/ext"]

def runtime_build(version, interpreter = "php", pcov = False):
    """Returns the BUILD file of one runtime repository; pcov names the PCOV extension it can load."""
    return _RUNTIME_BUILD.format(
        interpreter = interpreter,
        pcov = repr(PCOV_LABEL) if pcov else "None",
        php_bzl = str(Label("//php/toolchain:php.bzl")),
        version = version,
        version_file = VERSION_FILE,
    )

def hub_build(cells):
    """Returns the hub BUILD file: a runtime and an exec toolchain per cell."""
    content = []
    for cell in sorted(cells):
        constraints = PHP_CELLS[cell].exec_compatible_with
        content.append("\n".join([
            toolchain_target(
                name = cell,
                target_compatible_with = constraints,
                toolchain = "@{}//:php_toolchain".format(RUNTIME_PREFIX + cell),
                toolchain_type = TOOLCHAIN_TYPE,
            ),
            toolchain_target(
                name = cell + "_exec",
                exec_compatible_with = constraints,
                toolchain = "@{}//:php_toolchain".format(RUNTIME_PREFIX + cell),
                toolchain_type = EXEC_TOOLCHAIN_TYPE,
            ),
        ]))
    return "\n".join(content)

_SOURCES_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "files",
    srcs = glob(["src/**", "config/**", "rules.neon"], allow_empty = True),
)
"""

def phar_build(file):
    """Returns the BUILD file that exports one downloaded phar."""
    return 'package(default_visibility = ["//visibility:public"])\n\nexports_files([{}])\n'.format(repr(file))

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
            files = {"BUILD.bazel": _HEADERS_BUILD.format(includes = repr(_PHP_INCLUDES))},
            inner = [headers.inner],
            required = ["include/php/main/php_config.h"],
            sha256 = headers.sha256,
            type = "zip",
            urls = headers.urls,
        )
    pinned_archive(
        name = "dx_php_pcre2",
        files = {"BUILD.bazel": _HEADERS_BUILD.format(includes = repr(["include"]))},
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
            files = {"BUILD.bazel": _SOURCES_BUILD},
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
