"""The fpm hub: one repository whose packages are the locked fpm archives."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_FORTRAN_DEFS = str(Label("//fortran/private:defs.bzl"))

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    return "\n".join([
        'load("%s", "FORTRAN_EXTENSIONS", "fortran_library")' % _FORTRAN_DEFS,
        "",
        "fortran_library(",
        '    name = "%s",' % name,
        '    srcs = glob(["src/**/*." + ext for ext in FORTRAN_EXTENSIONS], allow_empty = True),',
        '    includes = ["src"],',
        "    deps = %s," % json.encode(["//" + dep for dep in package.get("deps", [])]),
        "    warnings_as_errors = False,",
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _fpm_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

fpm = module_extension(
    implementation = _fpm_impl,
    tag_classes = {"lock": lock_tag},
)
