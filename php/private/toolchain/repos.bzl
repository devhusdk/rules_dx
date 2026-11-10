"""Hermetic PHP, Composer and PHPUnit acquisition and toolchain registration."""

load("//core:toolchain.bzl", "toolchain_target")
load(":cells.bzl", "PHP_CELLS")

HUB_NAME = "dx_php_toolchains"

RUNTIME_PREFIX = "dx_php_"

VERSION_FILE = "DX_PHP_VERSION"

TOOLCHAIN_TYPE = str(Label("//php/private/toolchain:toolchain_type"))

EXEC_TOOLCHAIN_TYPE = str(Label("//php/private/toolchain:exec_toolchain_type"))

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

HEADERS_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

cc_library(
    name = "headers",
    hdrs = glob(["include/**/*.h"]),
    includes = {includes},
)
"""

def runtime_build(version, interpreter = "php", pcov = False):
    """Returns the BUILD file of one runtime repository; pcov names the PCOV extension it can load."""
    return _RUNTIME_BUILD.format(
        interpreter = interpreter,
        pcov = repr(PCOV_LABEL) if pcov else "None",
        php_bzl = str(Label("//php/private/toolchain:php.bzl")),
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

SOURCES_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "files",
    srcs = glob(["src/**", "config/**", "rules.neon"], allow_empty = True),
)
"""

def phar_build(file):
    """Returns the BUILD file that exports one downloaded phar."""
    return 'package(default_visibility = ["//visibility:public"])\n\nexports_files([{}])\n'.format(repr(file))
