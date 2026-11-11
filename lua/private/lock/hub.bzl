"""The rocks hub: one repository whose packages are built from the lock."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_LUA_DEFS = str(Label("//lua:defs.bzl"))

def _list(items):
    return "[" + ", ".join(['"%s"' % item for item in items]) + "]"

def _root(modules):
    module, path = sorted(modules.items())[0]
    return path[:len(path) - len(module.replace(".", "/") + ".lua")].rstrip("/")

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    name = package["name"]
    modules = package["extra"]["lua_modules"]
    root = _root(modules)
    return "\n".join([
        'load("%s", "lua_library")' % _LUA_DEFS,
        "",
        "lua_library(",
        '    name = "%s",' % name,
        "    srcs = %s," % _list(sorted(modules.values())),
        '    strip_import_prefix = "/%s",' % "/".join([part for part in [name, root] if part]),
        "    deps = %s," % _list(["//" + dep for dep in package.get("deps", [])]),
        '    visibility = ["//visibility:public"],',
        ")",
        "",
    ])

def _luarocks_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

luarocks = module_extension(
    implementation = _luarocks_impl,
    tag_classes = {"lock": lock_tag},
)
