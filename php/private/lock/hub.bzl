"""The composer hub: one repository whose packages are built from the lock."""

load(":repo.bzl", "lock_repos", "lock_tag")

_PHP_BZL = str(Label("//php/private:php.bzl"))

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    return "\n".join([
        'load("%s", "php_composer_package")' % _PHP_BZL,
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        "php_composer_package(",
        '    name = "%s",' % name.split("/")[-1],
        '    srcs = glob(["**"], exclude = ["BUILD.bazel", "REPO.bazel", "MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel"]),',
        "    manifest_json = %s," % repr(json.encode(package["extra"]["composer_manifest"])),
        "    package = %s," % repr(name),
        "    version = %s," % repr(package["version"]),
        "    deps = [%s]," % ", ".join(['"//%s"' % dep for dep in package.get("deps", [])]),
        ")",
        "",
    ])

def _composer_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

composer = module_extension(
    implementation = _composer_impl,
    tag_classes = {"lock": lock_tag},
)
