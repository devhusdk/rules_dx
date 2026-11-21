"""Module extension for php/private/toolchain."""

load("//cli/prebuilt:launcher.bzl", "FROM_SOURCE", "native_launcher_repos")
load(":cells.bzl", "PHP_CELLS")
load(":repos.bzl", "HUB_NAME", "SOURCES_BUILD", "VERSION_FILE", "hub_build", "phar_build", "php_archive", "php_hub", "runtime_build", "runtime_repository_name")

def _php_impl(mctx):
    pins = json.decode(mctx.read(Label("//php:pins.json")))
    php_version = pins["version"]
    tools = pins["tools"]
    for cell in sorted(PHP_CELLS.keys()):
        php_archive(
            name = runtime_repository_name(cell),
            files = {
                "BUILD.bazel": runtime_build(php_version, PHP_CELLS[cell].interpreter),
                VERSION_FILE: php_version + "\n",
            },
            required = [PHP_CELLS[cell].interpreter],
            sha256 = pins["cells"][cell]["sha256"],
            urls = pins["cells"][cell]["urls"],
        )
    php_hub(name = HUB_NAME, files = {"BUILD.bazel": hub_build(PHP_CELLS.keys())})
    for name, file, pin in [
        ("php_composer", "composer.phar", tools["composer"]),
        ("php_phpunit", "phpunit.phar", tools["phpunit"]),
        ("dx_phpbench", "phpbench.phar", tools["phpbench"]),
        ("dx_phpcs", "phpcs.phar", tools["phpcs"]),
        ("dx_phpdocumentor", "phpDocumentor.phar", tools["phpdocumentor"]),
        ("dx_phpstan", "phpstan.phar", tools["phpstan"]),
    ]:
        php_archive(name = name, download_as = file, files = {"BUILD.bazel": phar_build(file)}, sha256 = pin["sha256"], urls = pin["urls"])
    for name, pin in [
        ("dx_unused_public", tools["unused_public"]),
        ("dx_webmozart_assert", tools["webmozart_assert"]),
        ("dx_phpstan_strict_rules", tools["phpstan_strict_rules"]),
    ]:
        php_archive(
            name = name,
            files = {"BUILD.bazel": SOURCES_BUILD},
            required = ["src"],
            sha256 = pin["sha256"],
            strip_prefix = pin["strip_prefix"],
            urls = pin["urls"],
        )
    native_launcher_repos(name = "php_launcher", modules = mctx.modules, ext = "php")
    return mctx.extension_metadata(reproducible = True)

php = module_extension(
    implementation = _php_impl,
    doc = "Creates the pinned PHP runtimes, Composer, PHPUnit, the native launcher and the toolchain hub.",
    tag_classes = {"from_source": FROM_SOURCE},
)
