"""Module extension for the PHP coverage runtimes: the Windows coverage PHP and the PCOV source build."""

load("//php/private/toolchain:repos.bzl", "php_archive")
load(":repos.bzl", "COVERAGE_PREFIX", "HEADERS_BUILD", "php_coverage_runtime")

_PHP_INCLUDES = ["include/php", "include/php/main", "include/php/Zend", "include/php/TSRM", "include/php/ext"]

def _php_coverage_impl(mctx):
    pins = json.decode(mctx.read(Label("//php:pins.json")))
    pcov = pins["pcov"]
    pcre2 = pins["pcre2"]
    for cell, coverage in sorted(pins["coverage"].items()):
        php_coverage_runtime(
            name = COVERAGE_PREFIX + cell,
            pcov = [coverage["pcov"]["sha256"]] + coverage["pcov"]["urls"],
            php = [coverage["php"]["sha256"]] + coverage["php"]["urls"],
        )
    for cell, headers in sorted(pins["headers"].items()):
        php_archive(
            name = "dx_php_headers_" + cell,
            files = {"BUILD.bazel": HEADERS_BUILD.format(includes = repr(_PHP_INCLUDES))},
            inner = [headers["inner"]],
            required = ["include/php/main/php_config.h"],
            sha256 = headers["sha256"],
            type = "zip",
            urls = headers["urls"],
        )
    php_archive(
        name = "dx_php_pcre2",
        files = {"BUILD.bazel": HEADERS_BUILD.format(includes = repr(["include"]))},
        inner = [pcre2["inner"]],
        required = ["include/pcre2.h"],
        sha256 = pcre2["sha256"],
        type = "zip",
        urls = pcre2["urls"],
    )
    php_archive(
        name = "dx_php_pcov",
        files = {"BUILD.bazel": mctx.read(Label("//php/private:pcov.BUILD.bazel"))},
        patches = [Label(":patches/pcov_user_opcode_handlers.patch")],
        required = ["pcov-" + pcov["version"] + "/pcov.c"],
        sha256 = pcov["sha256"],
        urls = pcov["urls"],
    )
    return mctx.extension_metadata(reproducible = True)

php_coverage = module_extension(
    implementation = _php_coverage_impl,
    doc = "Creates the Windows coverage PHP and the PCOV source build with its PHP and PCRE2 headers.",
)
