"""Hermetic PHP, Composer and PHPUnit acquisition and toolchain registration."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":pins.bzl", "COMPOSER", "PHPCS", "PHPSTAN", "PHPUNIT", "PHP_CELLS", "PHP_VERSION")

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
    version = "{version}",
)
"""

_TOOLCHAINS = """toolchain(
    name = "{cell}",
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:php_toolchain",
    toolchain_type = "{toolchain_type}",
)

toolchain(
    name = "{cell}_exec",
    exec_compatible_with = {constraints},
    toolchain = "@{repo}//:php_toolchain",
    toolchain_type = "{exec_toolchain_type}",
)
"""

def runtime_build(version, interpreter = "php"):
    """Returns the BUILD file of one runtime repository."""
    return _RUNTIME_BUILD.format(
        interpreter = interpreter,
        php_bzl = str(Label("//php/toolchain:php.bzl")),
        version = version,
        version_file = VERSION_FILE,
    )

def hub_build(cells):
    """Returns the hub BUILD file: a runtime and an exec toolchain per cell."""
    content = []
    for cell in sorted(cells):
        content.append(_TOOLCHAINS.format(
            cell = cell,
            constraints = repr(PHP_CELLS[cell].exec_compatible_with),
            exec_toolchain_type = EXEC_TOOLCHAIN_TYPE,
            repo = RUNTIME_PREFIX + cell,
            toolchain_type = TOOLCHAIN_TYPE,
        ))
    return "\n".join(content)

def phar_build(file):
    """Returns the BUILD file that exports one downloaded phar."""
    return 'package(default_visibility = ["//visibility:public"])\n\nexports_files([{}])\n'.format(repr(file))

def _php_impl(mctx):
    for cell in sorted(PHP_CELLS.keys()):
        pinned_archive(
            name = RUNTIME_PREFIX + cell,
            files = {
                "BUILD.bazel": runtime_build(PHP_VERSION, PHP_CELLS[cell].interpreter),
                VERSION_FILE: PHP_VERSION + "\n",
            },
            required = [PHP_CELLS[cell].interpreter],
            sha256 = PHP_CELLS[cell].sha256,
            urls = PHP_CELLS[cell].urls,
        )
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(PHP_CELLS.keys())})
    for name, file, pin in [
        ("dx_composer", "composer.phar", COMPOSER),
        ("dx_phpunit", "phpunit.phar", PHPUNIT),
        ("dx_phpcs", "phpcs.phar", PHPCS),
        ("dx_phpstan", "phpstan.phar", PHPSTAN),
    ]:
        pinned_archive(name = name, download_as = file, files = {"BUILD.bazel": phar_build(file)}, sha256 = pin.sha256, urls = pin.urls)
    return mctx.extension_metadata(reproducible = True)

php = module_extension(
    implementation = _php_impl,
    doc = "Creates the pinned PHP runtimes, Composer, PHPUnit and the toolchain hub.",
)
