"""PHP toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//php/rules:defs.bzl", "PHP_COMPATIBLE_WITH")
load("//php/rules:php.bzl", "LAUNCHER_ATTRS", "lit", "rf", "runfiles_path", "write_launcher")
load(":php.bzl", "TOOLCHAIN_TYPE")
load(":pins.bzl", "COMPOSER", "PHPCS", "PHPSTAN", "PHPUNIT", "PHP_CELLS", "PHP_VERSION", "STATIC_PHP")
load(":repos.bzl", "VERSION_FILE", "hub_build", "runtime_build")

def _checks():
    hub = hub_build(PHP_CELLS.keys())
    archives = {
        "linux_amd64": "php-8.5.12-cli-linux-x86_64.tar.gz",
        "linux_arm64": "php-8.5.12-cli-linux-aarch64.tar.gz",
        "macos_arm64": "php-8.5.12-cli-macos-aarch64.tar.gz",
        "windows_x86_64": "php-8.5.12-cli-win.zip",
    }
    flavors = {"linux_amd64": "common", "linux_arm64": "common", "macos_arm64": "common", "windows_x86_64": "spc-max"}
    mac = PHP_CELLS["macos_arm64"]
    return [
        expect_equal("php version", PHP_VERSION, "8.5.12"),
        expect_equal("pinned cells", sorted(PHP_CELLS.keys()), ["linux_amd64", "linux_arm64", "macos_arm64", "windows_x86_64"]),
        expect_equal("archives", {c: PHP_CELLS[c].archive for c in PHP_CELLS}, archives),
        expect_true("cell digests", all([len(PHP_CELLS[c].sha256) == 64 for c in PHP_CELLS])),
        expect_true("cell urls", all([PHP_CELLS[c].urls == [STATIC_PHP + flavors[c] + "/" + PHP_CELLS[c].archive] for c in PHP_CELLS])),
        expect_equal("macos constraints", mac.exec_compatible_with, ["@platforms//os:macos", "@platforms//cpu:aarch64"]),
        expect_equal("windows constraints", PHP_CELLS["windows_x86_64"].exec_compatible_with, ["@platforms//os:windows", "@platforms//cpu:x86_64"]),
        expect_equal("windows interpreter", PHP_CELLS["windows_x86_64"].interpreter, "php.exe"),
        expect_equal("unix interpreters", [PHP_CELLS[c].interpreter for c in ["linux_amd64", "linux_arm64", "macos_arm64"]], ["php", "php", "php"]),
        expect_contains("the windows runtime exports its interpreter", runtime_build(PHP_VERSION, "php.exe"), 'interpreter = "php.exe",'),
        expect_contains("the windows runtime exports the exe", runtime_build(PHP_VERSION, "php.exe"), 'exports_files(["php.exe", "DX_PHP_VERSION"])'),
        expect_equal("composer version", COMPOSER.version, "2.10.3"),
        expect_equal("composer digest", len(COMPOSER.sha256), 64),
        expect_true("composer urls name the version", all([COMPOSER.version in url for url in COMPOSER.urls])),
        expect_equal("phpunit version", PHPUNIT.version, "13.4.3"),
        expect_equal("phpunit digest", len(PHPUNIT.sha256), 64),
        expect_true("phpunit urls name the version", all([PHPUNIT.version in url for url in PHPUNIT.urls])),
        expect_equal("phpcs version", PHPCS.version, "4.0.4"),
        expect_equal("phpcs digest", len(PHPCS.sha256), 64),
        expect_true("phpcs urls name the version", all([PHPCS.version in url for url in PHPCS.urls])),
        expect_equal("phpstan version", PHPSTAN.version, "2.3.3"),
        expect_equal("phpstan digest", len(PHPSTAN.sha256), 64),
        expect_true("phpstan urls name the version", all([PHPSTAN.version in url for url in PHPSTAN.urls])),
        expect_equal("the version file is named", VERSION_FILE, "DX_PHP_VERSION"),
        expect_contains("the runtime repo pins the version", runtime_build(PHP_VERSION), 'version = "8.5.12",'),
        expect_equal("a cell declares two toolchains", hub.count("toolchain("), 2 * len(PHP_CELLS)),
        expect_contains("the runtime toolchain matches the target", hub, "target_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_amd64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:toolchain_type")) + "\","),
        expect_contains("the exec toolchain matches the exec platform", hub, "exec_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_amd64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:exec_toolchain_type")) + "\","),
        expect_contains("the windows runtime toolchain matches the target", hub, "target_compatible_with = [\"@platforms//os:windows\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_windows_x86_64//:php_toolchain\","),
        expect_contains("the macos exec toolchain matches the exec platform", hub, "exec_compatible_with = [\"@platforms//os:macos\", \"@platforms//cpu:aarch64\"],\n    toolchain = \"@dx_php_macos_arm64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:exec_toolchain_type")) + "\","),
    ]

def php_metadata_tests(name):
    """Declares the PHP toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )

REQUIRED_MODULES = ["Phar", "dom", "json", "libxml", "mbstring", "tokenizer", "xml", "xmlwriter"]

def _php_version_test_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    script = ctx.file._script
    tokens = [rf(runfiles_path(ctx.workspace_name, script)), lit(PHP_VERSION)] + [lit(module) for module in REQUIRED_MODULES]
    launcher, runfiles = write_launcher(ctx, php, {}, tokens)
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles.merge(ctx.runfiles(files = [script], transitive_files = php.files)))]

_php_version_test = rule(
    implementation = _php_version_test_impl,
    doc = "Checks that the resolved PHP reports the pinned version and has the extensions Composer and PHPUnit need.",
    test = True,
    attrs = LAUNCHER_ATTRS | {
        "_script": attr.label(default = Label("//php/toolchain:version_check.php"), allow_single_file = True),
    },
    toolchains = [TOOLCHAIN_TYPE],
)

def php_version_test(name, **kwargs):
    """Declares the resolved-PHP version test on the qualified cell."""
    _php_version_test(name = name, target_compatible_with = PHP_COMPATIBLE_WITH, **kwargs)
