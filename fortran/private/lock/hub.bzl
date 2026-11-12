"""The fpm hub: one repository whose packages are the locked fpm archives."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_FORTRAN_DEFS = str(Label("//fortran/private:defs.bzl"))

_EXTENSIONS = "{f,f90,f95,f03,f08,F,F90,F95,F03,F08}"

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    return "\n".join([
        'load("%s", "fortran_library")' % _FORTRAN_DEFS,
        "",
        "fortran_library(",
        '    name = "%s",' % name,
        '    srcs = glob(["src/**/*.%s"], allow_empty = True),' % _EXTENSIONS,
        '    includes = ["src"],',
        "    deps = %s," % json.encode(["//" + dep for dep in package.get("deps", [])]),
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
