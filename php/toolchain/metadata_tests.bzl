"""PHP toolchain pin and acquisition tests."""

load("//libs/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS")
load("//libs/starlark:cells.bzl", "cell_cases", "cell_setting")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//php/rules:defs.bzl", "PHP_COMPATIBLE_WITH")
load("//php/rules:php.bzl", "lit", "rf", "runfiles_path", "write_launcher")
load(":php.bzl", "TOOLCHAIN_TYPE")
load(":pins.bzl", "COMPOSER", "PCOV", "PHPBENCH", "PHPCS", "PHPSTAN", "PHPSTAN_STRICT_RULES", "PHPUNIT", "PHP_CELLS", "PHP_HEADERS", "PHP_VERSION", "STATIC_PHP", "UNUSED_PUBLIC", "WEBMOZART_ASSERT")
load(":repos.bzl", "VERSION_FILE", "hub_build", "runtime_build")

PINS = [COMPOSER, PCOV, PHPBENCH, PHPCS, PHPSTAN, PHPSTAN_STRICT_RULES, PHPUNIT, UNUSED_PUBLIC, WEBMOZART_ASSERT]

def _checks():
    hub = hub_build(PHP_CELLS.keys())
    flavors = {"linux_x86_64": "gnu-bulk", "linux_arm64": "gnu-bulk", "macos_arm64": "common", "windows_x86_64": "spc-max"}
    mac = PHP_CELLS["macos_arm64"]
    cases = cell_cases("//php/toolchain", PHP_CELLS.keys(), unsupported = True)
    default = cases.pop("//conditions:default")
    return [
        expect_true("pinned tools carry a digest and name their version", all([len(pin.sha256) == 64 and all([pin.version in url for url in pin.urls]) for pin in PINS])),
        expect_equal("one case per pinned cell", sorted([str(key) for key in cases]), sorted([str(cell_setting("//php/toolchain", cell)) for cell in PHP_CELLS])),
        expect_equal("pinned cells allow the target", cases.values(), [[] for _ in PHP_CELLS]),
        expect_equal("other platforms are incompatible and unsupported", [str(label) for label in default], [str(Label("@platforms//:incompatible")), str(Label("//php/toolchain:unsupported"))]),
        expect_true("cell digests", all([len(PHP_CELLS[c].sha256) == 64 for c in PHP_CELLS])),
        expect_true("cell urls", all([PHP_CELLS[c].urls == [STATIC_PHP + flavors[c] + "/" + PHP_CELLS[c].archive] for c in PHP_CELLS])),
        expect_equal("macos constraints", mac.exec_compatible_with, ["@platforms//os:macos", "@platforms//cpu:aarch64"]),
        expect_equal("windows constraints", PHP_CELLS["windows_x86_64"].exec_compatible_with, ["@platforms//os:windows", "@platforms//cpu:x86_64"]),
        expect_equal("windows interpreter", PHP_CELLS["windows_x86_64"].interpreter, "php.exe"),
        expect_equal("unix interpreters", [PHP_CELLS[c].interpreter for c in ["linux_arm64", "linux_x86_64", "macos_arm64"]], ["php", "php", "php"]),
        expect_contains("a pcov runtime names the extension", runtime_build(PHP_VERSION, "php", True), 'pcov = "@dx_php_pcov//:pcov.so",'),
        expect_contains("a runtime without pcov names none", runtime_build(PHP_VERSION), "pcov = None,"),
        expect_equal("only the linux cells load pcov", sorted([c for c in PHP_CELLS if PHP_CELLS[c].pcov]), ["linux_arm64", "linux_x86_64"]),
        expect_true("pcov headers are pinned per linux cell", all([len(PHP_HEADERS[c].sha256) == 64 for c in PHP_HEADERS]) and sorted(PHP_HEADERS.keys()) == ["linux_arm64", "linux_x86_64"]),
        expect_contains("the windows runtime exports its interpreter", runtime_build(PHP_VERSION, "php.exe"), 'interpreter = "php.exe",'),
        expect_contains("the windows runtime exports the exe", runtime_build(PHP_VERSION, "php.exe"), 'exports_files(["php.exe", "DX_PHP_VERSION"])'),
        expect_equal("the version file is named", VERSION_FILE, "DX_PHP_VERSION"),
        expect_contains("the runtime repo pins the version", runtime_build(PHP_VERSION), 'version = "' + PHP_VERSION + '",'),
        expect_equal("a cell declares two toolchains", hub.count("toolchain("), 2 * len(PHP_CELLS)),
        expect_contains("the runtime toolchain matches the target", hub, "target_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_x86_64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:toolchain_type")) + "\","),
        expect_contains("the exec toolchain matches the exec platform", hub, "exec_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_x86_64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/toolchain:exec_toolchain_type")) + "\","),
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
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "_script": attr.label(default = Label("//php/toolchain:version_check.php"), allow_single_file = True),
    },
    toolchains = [TOOLCHAIN_TYPE],
)

def php_version_test(name, **kwargs):
    """Declares the resolved-PHP version test on the qualified cell."""
    kwargs.setdefault("size", "small")
    _php_version_test(name = name, target_compatible_with = PHP_COMPATIBLE_WITH, **kwargs)
