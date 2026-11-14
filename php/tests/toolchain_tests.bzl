"""PHP toolchain pin and acquisition tests."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS")
load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//php/private:defs.bzl", "PHP_COMPATIBLE_WITH")
load("//php/private:php.bzl", "lit", "rf", "runfiles_path", "write_launcher")
load("//php/private/toolchain:php.bzl", "TOOLCHAIN_TYPE")
load("//php/private/toolchain:cells.bzl", "PHP_CELLS", "cell_cases", "cell_setting")
load("//php/private/toolchain:repos.bzl", "COVERAGE_BUILD", "COVERAGE_PREFIX", "VERSION_FILE", "hub_build", "runtime_build")

_PHP = PINS["dx_php"]

PHP_VERSION = _PHP["version"]

STATIC_PHP = "https://dl.static-php.dev/v3/php-bin/"

MIRROR = "https://github.com/devhusdk/rules_dx/releases/download/mirror-php-" + PHP_VERSION + "/"

def _checks():
    hub = hub_build(PHP_CELLS.keys())
    flavors = {"linux_x86_64": "gnu-bulk", "linux_arm64": "gnu-bulk", "macos_arm64": "bulk", "windows_x86_64": "spc-max"}
    mac = PHP_CELLS["macos_arm64"]
    pins = [_PHP["pcov"]] + _PHP["tools"].values()
    cells = _PHP["cells"]
    cases = cell_cases("//php/private/toolchain", PHP_CELLS.keys())
    default = cases.pop("//conditions:default")
    return [
        expect_true("pinned tools carry a digest and name their version", all([len(pin["sha256"]) == 64 and all([pin["version"] in url for url in pin["urls"]]) for pin in pins])),
        expect_equal("one case per pinned cell", sorted([str(key) for key in cases]), sorted([str(cell_setting("//php/private/toolchain", cell)) for cell in PHP_CELLS])),
        expect_equal("pinned cells allow the target", cases.values(), [[] for _ in PHP_CELLS]),
        expect_equal("other platforms are incompatible and unsupported", [str(label) for label in default], [str(Label("@platforms//:incompatible")), str(Label("//php/private/toolchain:unsupported"))]),
        expect_equal("pinned cells are the language cells", sorted(cells.keys()), sorted(PHP_CELLS.keys())),
        expect_true("cell urls", all([cells[c]["urls"][0].startswith(MIRROR) and cells[c]["urls"][1].startswith(STATIC_PHP + flavors[c] + "/") for c in PHP_CELLS])),
        expect_equal("cell binaries are the interpreters", {c: cells[c]["binary"] for c in cells}, {c: PHP_CELLS[c].interpreter for c in PHP_CELLS}),
        expect_equal("macos constraints", mac.exec_compatible_with, ["@platforms//os:macos", "@platforms//cpu:aarch64"]),
        expect_equal("windows constraints", PHP_CELLS["windows_x86_64"].exec_compatible_with, ["@platforms//os:windows", "@platforms//cpu:x86_64"]),
        expect_equal("windows interpreter", PHP_CELLS["windows_x86_64"].interpreter, "php.exe"),
        expect_equal("unix interpreters", [PHP_CELLS[c].interpreter for c in ["linux_arm64", "linux_x86_64", "macos_arm64"]], ["php", "php", "php"]),
        expect_contains("a pcov runtime names the extension", runtime_build(PHP_VERSION, "php", True), 'pcov = "@dx_php_pcov//:pcov.so",'),
        expect_contains("a runtime without pcov names none", runtime_build(PHP_VERSION), "pcov = None,"),
        expect_equal("every cell loads pcov", sorted([c for c in PHP_CELLS if PHP_CELLS[c].pcov]), ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]),
        expect_equal("windows has a separate coverage runtime", sorted(_PHP["coverage"].keys()), ["windows_x86_64"]),
        expect_true("coverage runtimes are pinned windows.php.net builds", all([len(pin["sha256"]) == 64 and all([url.startswith("https://downloads.php.net/~windows/") for url in pin["urls"]]) for c in _PHP["coverage"].values() for pin in [c["php"], c["pcov"]]])),
        expect_true("the coverage php matches the version", all([PHP_VERSION + "-" in url and "-nts-" in url for c in _PHP["coverage"].values() for url in c["php"]["urls"]])),
        expect_true("the coverage pcov matches the pcov version", all([c["pcov"]["version"] == _PHP["pcov"]["version"] and all([c["pcov"]["version"] in url and "-" + PHP_VERSION.rsplit(".", 1)[0] + "-nts-" in url for url in c["pcov"]["urls"]]) for c in _PHP["coverage"].values()])),
        expect_contains("the windows runtime loads pcov from its coverage runtime", runtime_build(PHP_VERSION, "php.exe", True, COVERAGE_PREFIX + "windows_x86_64"), 'pcov = "@dx_php_coverage_windows_x86_64//:php_pcov.dll",'),
        expect_contains("the windows runtime names the coverage interpreter", runtime_build(PHP_VERSION, "php.exe", True, COVERAGE_PREFIX + "windows_x86_64"), 'coverage_interpreter = "@dx_php_coverage_windows_x86_64//:php.exe",'),
        expect_contains("the windows coverage runtime loads mbstring", runtime_build(PHP_VERSION, "php.exe", True, COVERAGE_PREFIX + "windows_x86_64"), 'coverage_extensions = ["@dx_php_coverage_windows_x86_64//:ext/php_mbstring.dll"],'),
        expect_contains("the coverage runtime ships the msvc runtime beside php", COVERAGE_BUILD, '"php.exe", "php8.dll", "ext/php_mbstring.dll", "php_pcov.dll", "vcruntime140.dll"'),
        expect_true("pcov headers are pinned per unix cell", all([len(h["sha256"]) == 64 for h in _PHP["headers"].values()]) and sorted(_PHP["headers"].keys()) == ["linux_arm64", "linux_x86_64", "macos_arm64"]),
        expect_contains("the windows runtime exports its interpreter", runtime_build(PHP_VERSION, "php.exe"), 'interpreter = "php.exe",'),
        expect_contains("the windows runtime exports the exe", runtime_build(PHP_VERSION, "php.exe"), 'exports_files(["php.exe", "DX_PHP_VERSION"])'),
        expect_equal("the version file is named", VERSION_FILE, "DX_PHP_VERSION"),
        expect_contains("the runtime repo pins the version", runtime_build(PHP_VERSION), 'version = "' + PHP_VERSION + '",'),
        expect_equal("a cell declares two toolchains", hub.count("toolchain("), 2 * len(PHP_CELLS)),
        expect_contains("the runtime toolchain matches the target", hub, "target_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_x86_64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/private/toolchain:toolchain_type")) + "\","),
        expect_contains("the exec toolchain matches the exec platform", hub, "exec_compatible_with = [\"@platforms//os:linux\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_linux_x86_64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/private/toolchain:exec_toolchain_type")) + "\","),
        expect_contains("the windows runtime toolchain matches the target", hub, "target_compatible_with = [\"@platforms//os:windows\", \"@platforms//cpu:x86_64\"],\n    toolchain = \"@dx_php_windows_x86_64//:php_toolchain\","),
        expect_contains("the macos exec toolchain matches the exec platform", hub, "exec_compatible_with = [\"@platforms//os:macos\", \"@platforms//cpu:aarch64\"],\n    toolchain = \"@dx_php_macos_arm64//:php_toolchain\",\n    toolchain_type = \"" + str(Label("//php/private/toolchain:exec_toolchain_type")) + "\","),
    ]

def php_metadata_tests(name, size):
    """Declares the PHP toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )

REQUIRED_MODULES = ["Phar", "ctype", "dom", "hash", "iconv", "json", "libxml", "mbstring", "simplexml", "tokenizer", "xml", "xmlwriter"]

def _php_version_test_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    script = ctx.file._script
    tokens = [rf(runfiles_path(ctx.workspace_name, script.short_path)), lit(PHP_VERSION)] + [lit(module) for module in REQUIRED_MODULES]
    launcher, runfiles = write_launcher(ctx, php, {}, tokens)
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles.merge(ctx.runfiles(files = [script], transitive_files = php.files)))]

_php_version_test = rule(
    implementation = _php_version_test_impl,
    doc = "Checks that the resolved PHP reports the pinned version and has the extensions Composer and PHPUnit need.",
    test = True,
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "_script": attr.label(default = Label("//php/private/toolchain:version_check.php"), allow_single_file = True),
    },
    toolchains = [TOOLCHAIN_TYPE],
)

def php_version_test(name, **kwargs):
    """Declares the resolved-PHP version test on the qualified cell."""
    kwargs.setdefault("size", "small")
    _php_version_test(name = name, target_compatible_with = PHP_COMPATIBLE_WITH, **kwargs)
