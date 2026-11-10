"""The swiftpm hub: one repository whose packages are built from the lock."""

load("//core:pins.bzl", "lock_repos", "lock_tag")

_SWIFT_DEFS = str(Label("//swift:defs.bzl"))

def _list(items):
    return "[" + ", ".join(['"%s"' % item for item in items]) + "]"

def build_file(package):
    """Returns the BUILD file text of a decoded lock package."""
    extra = package["extra"]
    lines = ['load("%s", "swift_library")' % _SWIFT_DEFS, ""]
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
        lines += [
            "swift_library(",
            '    name = "%s",' % name,
            '    module_name = "%s",' % target["module"],
            "    srcs = %s," % _list(target["srcs"]),
            "    deps = %s," % _list(deps),
            '    visibility = ["//visibility:public"],',
            ")",
            "",
        ]
    for product, targets in extra["swift_products"].items():
        if product in names:
            if targets[0] != product:
                fail("product %s of %s is also the name of another target" % (product, package["name"]))
        else:
            names[product] = True
            lines += [
                "alias(",
                '    name = "%s",' % product,
                '    actual = ":%s",' % targets[0],
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
