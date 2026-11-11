"""The dub hub: one repository whose packages are built from the lock."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_D_DEFS = str(Label("//d:defs.bzl"))

def _list(items):
    return "[" + ", ".join(['"%s"' % item for item in items]) + "]"

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    layout = package["extra"]["d_library"]
    string_imports = layout.get("string_imports", [])
    lines = [
        'load("%s", "d_library")' % _D_DEFS,
        "",
        "d_library(",
        '    name = "%s",' % name,
        "    srcs = glob(%s, allow_empty = True)," % _list(layout["srcs"]),
        "    deps = %s," % _list(["//" + dep for dep in package.get("deps", [])]),
        "    imports = %s," % _list(layout["imports"]),
    ]
    if string_imports:
        lines += [
            "    string_imports = %s," % _list(string_imports),
            "    string_srcs = glob(%s)," % _list(["%s/**" % path for path in string_imports]),
        ]
    lines += [
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ]
    return "\n".join(lines)

def _dub_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

dub = module_extension(
    implementation = _dub_impl,
    tag_classes = {"lock": lock_tag},
)
