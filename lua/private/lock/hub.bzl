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

def luarocks_problems(lock):
    """Returns why a decoded dx lock that `lock_problems` accepts is not a luarocks lock: one message per problem, empty when it is."""
    problems = []
    if lock.get("ecosystem") != "luarocks":
        problems.append("has ecosystem %r, not luarocks" % lock.get("ecosystem"))
    for package in lock["packages"]:
        name = package["name"]
        extra = package.get("extra", {})
        modules = extra.get("lua_modules") if type(extra) == "dict" else None
        if type(modules) != "dict" or not modules:
            problems.append("package %s has no lua_modules object" % name)
            continue
        for module, path in sorted(modules.items()):
            if type(path) != "string" or not ("/" + path).endswith("/" + module.replace(".", "/") + ".lua"):
                problems.append("package %s module %s has path %r, which does not end in %s.lua" % (name, module, path, module.replace(".", "/")))
    return problems

def _luarocks_impl(mctx):
    lock_repos(mctx, build_file, luarocks_problems)
    return mctx.extension_metadata(reproducible = True)

luarocks = module_extension(
    implementation = _luarocks_impl,
    tag_classes = {"lock": lock_tag},
)
