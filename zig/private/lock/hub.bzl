"""The zig hub: one repository whose packages are built from the lock."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_ZIG_DEFS = str(Label("//zig:defs.bzl"))

def _list(items):
    return "[" + ", ".join(['"%s"' % item for item in items]) + "]"

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    main = package["extra"]["zig_main"]
    return "\n".join([
        'load("%s", "zig_library")' % _ZIG_DEFS,
        "",
        "zig_library(",
        '    name = "%s",' % name,
        '    srcs = glob(["**/*.zig"], exclude = ["build.zig", "%s"], allow_empty = True),' % main,
        '    import_name = "%s",' % name,
        '    main = "%s",' % main,
        "    deps = %s," % _list(["//" + dep for dep in package.get("deps", [])]),
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _zig_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

zig = module_extension(
    implementation = _zig_impl,
    tag_classes = {"lock": lock_tag},
)
