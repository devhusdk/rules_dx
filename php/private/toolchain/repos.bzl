"""Hermetic PHP, Composer and PHPUnit acquisition and toolchain registration."""

load("//cc/private/toolchain:pins.bzl", "VC_REDIST_VERSION")
load("//core:repo.bzl", "digest_error")
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
    name = "php_toolchain",{coverage}
    interpreter = "{interpreter}",
    pcov = {pcov},
    version = "{version}",
)
"""

_COVERAGE_ATTRS = """
    coverage_extensions = ["@{repo}//:ext/php_mbstring.dll"],
    coverage_files = "@{repo}//:files",
    coverage_interpreter = "@{repo}//:php.exe","""

COVERAGE_PREFIX = "dx_php_coverage_"

COVERAGE_RUNTIME_FILES = ["php.exe", "php8.dll", "ext/php_mbstring.dll", "php_pcov.dll", "vcruntime140.dll"]

COVERAGE_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files({files})

filegroup(
    name = "files",
    srcs = {files},
)
""".format(files = repr(COVERAGE_RUNTIME_FILES))

PCOV_LABEL = "@dx_php_pcov//:pcov.so"

HEADERS_BUILD = """load("@rules_cc//cc:defs.bzl", "cc_library")

package(default_visibility = ["//visibility:public"])

cc_library(
    name = "headers",
    hdrs = glob(["include/**/*.h"]),
    includes = {includes},
)
"""

def runtime_build(version, interpreter = "php", pcov = False, coverage = ""):
    """Returns the BUILD file of one runtime repository; pcov names the PCOV extension it can load and coverage the repository of a separate coverage runtime."""
    if coverage:
        pcov_label = repr("@" + coverage + "//:php_pcov.dll")
    else:
        pcov_label = repr(PCOV_LABEL) if pcov else "None"
    return _RUNTIME_BUILD.format(
        coverage = _COVERAGE_ATTRS.format(repo = coverage) if coverage else "",
        interpreter = interpreter,
        pcov = pcov_label,
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

def _coverage_runtime_impl(rctx):
    for pin in [rctx.attr.php, rctx.attr.pcov]:
        sha256, urls = pin[0], pin[1:]
        error = digest_error(rctx.name, sha256, "")
        if error:
            fail(error)
        rctx.download_and_extract(url = urls, sha256 = sha256, type = "zip")
    redist = rctx.path(rctx.attr._msvc_runtime.same_package_label("BUILD.bazel")).dirname.get_child("sysroot/Contents/VC/Redist/MSVC/{}/x64".format(VC_REDIST_VERSION))
    if redist.exists:
        for directory in redist.readdir():
            for dll in directory.readdir():
                if dll.basename.lower() == "vcruntime140.dll":
                    rctx.symlink(dll, "vcruntime140.dll")
    for path in COVERAGE_RUNTIME_FILES:
        if not rctx.path(path).exists:
            fail(rctx.name + ": the coverage runtime has no " + path)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", COVERAGE_BUILD)
    return rctx.repo_metadata(reproducible = True)

php_coverage_runtime = repository_rule(
    implementation = _coverage_runtime_impl,
    doc = "Fetches a dynamic PHP with PCOV and places the MSVC runtime it links beside php.exe.",
    attrs = {
        "pcov": attr.string_list(mandatory = True, doc = "The PCOV zip: its sha256 and then its URLs."),
        "php": attr.string_list(mandatory = True, doc = "The PHP zip: its sha256 and then its URLs."),
        "_msvc_runtime": attr.label(default = "@msvc_runtime//:BUILD.bazel", doc = "The hermetic MSVC redistributable."),
    },
)
