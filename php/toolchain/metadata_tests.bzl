"""PHP toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":php.bzl", "TOOLCHAIN_TYPE")
load(":pins.bzl", "COMPOSER", "PHPSTAN", "PHPUNIT", "PHP_CELLS", "PHP_VERSION", "STATIC_PHP")
load(":repos.bzl", "VERSION_FILE", "hub_build", "runtime_build")

PHP_TEST_COMPATIBLE_WITH = [
    "@platforms//os:linux",
    "@platforms//cpu:x86_64",
]

def _checks():
    linux = PHP_CELLS["linux_amd64"]
    hub = hub_build(["linux_amd64"])
    return [
        expect_equal("php version", PHP_VERSION, "8.5.12"),
        expect_equal("pinned cells", sorted(PHP_CELLS.keys()), ["linux_amd64"]),
        expect_equal("linux archive", linux.archive, "php-8.5.12-cli-linux-x86_64.tar.gz"),
        expect_equal("linux digest", len(linux.sha256), 64),
        expect_equal("linux urls", linux.urls, [STATIC_PHP + linux.archive]),
        expect_equal("composer version", COMPOSER.version, "2.10.3"),
        expect_equal("composer digest", len(COMPOSER.sha256), 64),
        expect_true("composer urls name the version", all([COMPOSER.version in url for url in COMPOSER.urls])),
        expect_equal("phpunit version", PHPUNIT.version, "13.4.3"),
        expect_equal("phpunit digest", len(PHPUNIT.sha256), 64),
        expect_true("phpunit urls name the version", all([PHPUNIT.version in url for url in PHPUNIT.urls])),
        expect_equal("phpstan version", PHPSTAN.version, "2.3.3"),
        expect_equal("phpstan digest", len(PHPSTAN.sha256), 64),
        expect_true("phpstan urls name the version", all([PHPSTAN.version in url for url in PHPSTAN.urls])),
        expect_equal("the version file is named", VERSION_FILE, "DX_PHP_VERSION"),
        expect_contains("the runtime repo pins the version", runtime_build(PHP_VERSION), 'version = "8.5.12",'),
        expect_equal("a cell declares two toolchains", hub.count("toolchain("), 2),
        expect_contains("the runtime toolchain matches the target", hub, "target_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_amd64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:toolchain_type")) + "\","),
        expect_contains("the exec toolchain matches the exec platform", hub, "exec_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_amd64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:exec_toolchain_type")) + "\","),
    ]

def php_metadata_tests(name):
    """Declares the PHP toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )

_VERSION_SCRIPT = """#!/usr/bin/env bash
set -euo pipefail
runfiles="${{RUNFILES_DIR:-$0.runfiles}}"
got="$("$runfiles/{php}" -n -r 'echo PHP_VERSION;')"
if [[ "$got" != "{want}" ]]; then
    echo "resolved PHP is $got, the pin is {want}" >&2
    exit 1
fi
modules="$("$runfiles/{php}" -n -m)"
for module in {modules}; do
    if ! grep -qx "$module" <<<"$modules"; then
        echo "resolved PHP lacks the $module extension" >&2
        exit 1
    fi
done
"""

REQUIRED_MODULES = ["Phar", "dom", "json", "libxml", "mbstring", "tokenizer", "xml", "xmlwriter"]

def _runfiles_path(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def _php_version_test_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(script, _VERSION_SCRIPT.format(
        php = _runfiles_path(ctx, php.interpreter),
        want = PHP_VERSION,
        modules = " ".join(REQUIRED_MODULES),
    ), is_executable = True)
    return [DefaultInfo(executable = script, runfiles = ctx.runfiles(transitive_files = php.files))]

_php_version_test = rule(
    implementation = _php_version_test_impl,
    doc = "Checks that the resolved PHP reports the pinned version and has the extensions Composer and PHPUnit need.",
    test = True,
    toolchains = [TOOLCHAIN_TYPE],
)

def php_version_test(name, **kwargs):
    """Declares the resolved-PHP version test on the qualified cell."""
    _php_version_test(name = name, target_compatible_with = PHP_TEST_COMPATIBLE_WITH, **kwargs)
