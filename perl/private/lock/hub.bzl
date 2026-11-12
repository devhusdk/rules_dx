"""The cpan hub: one repository whose packages are the locked CPAN distributions."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_PACKAGE_BZL = str(Label("//perl/private/lock:package.bzl"))

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    return "\n".join([
        'load("%s", "cpan_package")' % _PACKAGE_BZL,
        "",
        "cpan_package(",
        '    name = "%s",' % package["name"],
        "    deps = [%s]," % ", ".join(['"//%s"' % dep for dep in package.get("deps", [])]),
        ")",
        "",
    ])

def _cpan_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

cpan = module_extension(
    implementation = _cpan_impl,
    tag_classes = {"lock": lock_tag},
)
