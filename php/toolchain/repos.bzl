"""Hermetic PHP, Composer and PHPUnit acquisition and toolchain registration."""

load("//erlang/toolchain:pins.bzl", "CELLS")
load(":pins.bzl", "COMPOSER", "PHPUNIT", "PHP_CELLS", "PHP_VERSION")

HUB_NAME = "dx_php_toolchains"

RUNTIME_PREFIX = "dx_php_"

VERSION_FILE = "DX_PHP_VERSION"

TOOLCHAIN_TYPE = str(Label("//php/toolchain:toolchain_type"))

EXEC_TOOLCHAIN_TYPE = str(Label("//php/toolchain:exec_toolchain_type"))

_RUNTIME_BUILD = """load("{php_bzl}", "php_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["php", "{version_file}"])

php_toolchain(
    name = "php_toolchain",
    interpreter = "php",
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

def runtime_build(version):
    """Returns the BUILD file of one runtime repository."""
    return _RUNTIME_BUILD.format(
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
            constraints = repr(CELLS[cell].exec_compatible_with),
            exec_toolchain_type = EXEC_TOOLCHAIN_TYPE,
            repo = RUNTIME_PREFIX + cell,
            toolchain_type = TOOLCHAIN_TYPE,
        ))
    return "\n".join(content)

def _runtime_impl(rctx):
    rctx.download_and_extract(url = rctx.attr.urls, sha256 = rctx.attr.sha256)
    if not rctx.path("php").exists:
        fail("PHP archive " + rctx.attr.archive + " has no php binary")
    rctx.file(VERSION_FILE, rctx.attr.version + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", runtime_build(rctx.attr.version))
    return rctx.repo_metadata(reproducible = True)

_runtime = repository_rule(
    implementation = _runtime_impl,
    doc = "Acquires one pinned static PHP CLI build.",
    attrs = {
        "archive": attr.string(mandatory = True, doc = "Release archive name."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "urls": attr.string_list(mandatory = True, doc = "Archive URLs, tried in order."),
        "version": attr.string(mandatory = True, doc = "PHP version the archive holds."),
    },
)

def _phar_impl(rctx):
    rctx.download(url = rctx.attr.urls, output = rctx.attr.file, sha256 = rctx.attr.sha256)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", 'package(default_visibility = ["//visibility:public"])\n\nexports_files([{}])\n'.format(repr(rctx.attr.file)))
    return rctx.repo_metadata(reproducible = True)

php_phar = repository_rule(
    implementation = _phar_impl,
    doc = "Downloads one pinned PHP archive (phar) and exports it as a file.",
    attrs = {
        "file": attr.string(mandatory = True, doc = "File name the phar is exported as."),
        "sha256": attr.string(mandatory = True, doc = "Phar digest."),
        "urls": attr.string_list(mandatory = True, doc = "Phar URLs, tried in order."),
    },
)

def _hub_impl(rctx):
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", hub_build(PHP_CELLS.keys()))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares the PHP runtime and exec toolchains per qualified cell.",
)

def _php_impl(mctx):
    for cell in sorted(PHP_CELLS.keys()):
        _runtime(
            name = RUNTIME_PREFIX + cell,
            archive = PHP_CELLS[cell].archive,
            sha256 = PHP_CELLS[cell].sha256,
            urls = PHP_CELLS[cell].urls,
            version = PHP_VERSION,
        )
    _hub(name = HUB_NAME)
    php_phar(name = "dx_composer", file = "composer.phar", sha256 = COMPOSER.sha256, urls = COMPOSER.urls)
    php_phar(name = "dx_phpunit", file = "phpunit.phar", sha256 = PHPUNIT.sha256, urls = PHPUNIT.urls)
    return mctx.extension_metadata(reproducible = True)

php = module_extension(
    implementation = _php_impl,
    doc = "Creates the pinned PHP runtimes, Composer, PHPUnit and the toolchain hub.",
)
