"""The swiftpm hub: one repository whose packages are built from the lock."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_SWIFT_DEFS = str(Label("//swift:defs.bzl"))
_CC_DEFS = str(Label("//cc:defs.bzl"))

_HEADER_EXTENSIONS = [".h", ".hh", ".hpp", ".hxx", ".inc"]

def _list(items):
    return "[" + ", ".join(['"%s"' % item for item in items]) + "]"

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    extra = package["extra"]
    lines = [
        'load("%s", "swift_interop_hint", "swift_library", "swift_library_group")' % _SWIFT_DEFS,
        'load("%s", "cc_library")' % _CC_DEFS,
        "",
    ]
    names = {}
    for name, target in extra["swift_targets"].items():
        names[name] = True
        if not target["srcs"]:
            fail("target %s of %s has no sources" % (name, package["name"]))
        for src in target["srcs"]:
            if not src.startswith(target["path"] + "/"):
                fail("source %s of target %s is outside %s" % (src, name, target["path"]))
        deps = [":" + dep for dep in target["deps"]]
        for product_dep in target["product_deps"]:
            identity, product = product_dep.split(":")
            deps.append("//%s:%s" % (identity, product))
        defines = target.get("defines", [])
        if target.get("kind") == "clang":
            include = target["include"]
            header_paths = target.get("header_paths", [])
            private = ['"%s/**/*.h"' % path for path in header_paths]
            lines += [
                "swift_interop_hint(",
                '    name = "%s_hint",' % name,
                '    module_name = "%s",' % target["module"],
                ")",
                "",
                "cc_library(",
                '    name = "%s",' % name,
                "    srcs = %s + glob([%s])," % (_list(target["srcs"]), ", ".join(private)),
                '    hdrs = glob([%s]),' % ", ".join(['"%s/**/*%s"' % (include, ext) for ext in _HEADER_EXTENSIONS]),
                "    defines = %s," % _list(defines),
                "    includes = %s," % _list([include] + header_paths),
                "    deps = %s," % _list(deps),
                '    aspect_hints = [":%s_hint"],' % name,
                '    visibility = ["//visibility:public"],',
                ")",
                "",
            ]
            continue
        lines += [
            "swift_library(",
            '    name = "%s",' % name,
            '    module_name = "%s",' % target["module"],
            "    srcs = %s," % _list(target["srcs"]),
            "    defines = %s," % _list(defines),
            "    deps = %s," % _list(deps),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    for product, targets in extra["swift_products"].items():
        if product in names:
            if targets != [product]:
                fail("product %s of %s is also the name of another target" % (product, package["name"]))
        elif len(targets) == 1:
            names[product] = True
            lines += [
                "alias(",
                '    name = "%s",' % product,
                '    actual = ":%s",' % targets[0],
                '    visibility = ["//visibility:public"],',
                ")",
                "",
            ]
        else:
            names[product] = True
            lines += [
                "swift_library_group(",
                '    name = "%s",' % product,
                "    deps = %s," % _list([":" + target for target in targets]),
                '    visibility = ["//visibility:public"],',
                ")",
                "",
            ]
    products = extra["swift_products"]
    if package["name"] not in names and len(products) == 1:
        lines += [
            "alias(",
            '    name = "%s",' % package["name"],
            '    actual = ":%s",' % products.keys()[0],
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    return "\n".join(lines)

def _swiftpm_impl(mctx):
    lock_repos(mctx, build_file)
    return mctx.extension_metadata(reproducible = True)

swiftpm = module_extension(
    implementation = _swiftpm_impl,
    tag_classes = {"lock": lock_tag},
)
