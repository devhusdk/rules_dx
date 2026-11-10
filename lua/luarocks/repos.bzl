"""Repository rule and hub for LuaRocks packages fetched from luarocks.lock."""

load("//core:pins.bzl", "download_pinned")
load("//core:repo.bzl", "hub_repo")
load(":lock.bzl", "analyze", "hub_package_build", "hub_root_build", "module_path", "pin_entry_json", "spoke_build")

_RULES_BZL = str(Label("//lua:defs.bzl"))

def _rock_impl(rctx):
    attrs = rctx.attr
    archive = "_archive_" + attrs.urls[0].split("/")[-1]
    download_pinned(rctx, archive, lambda sha256: pin_entry_json(attrs.package, sha256))
    rctx.extract(archive, output = "_src", strip_prefix = attrs.strip_prefix)
    rctx.delete(archive)
    srcs = []
    for module, path in sorted(attrs.modules.items()):
        source = rctx.path("_src/" + path)
        if not source.exists:
            fail("%s: module %s names %s, which is not in the source archive" % (attrs.package, module, path))
        dest = module_path(module)
        rctx.file(dest, rctx.read(source))
        srcs.append(dest)
    rctx.delete("_src")
    rctx.file("REPO.bazel", "")
    package = struct(name = attrs.package, modules = attrs.modules)
    dep_spokes = [(spoke, rock) for spoke, rock in zip(attrs.dep_spokes, attrs.dep_rocks)]
    rctx.file("BUILD.bazel", spoke_build(package, _RULES_BZL, dep_spokes))
    return rctx.repo_metadata(reproducible = True)

luarocks_package = repository_rule(
    implementation = _rock_impl,
    doc = "Downloads one locked pure-Lua rock and declares its modules as a lua_library.",
    attrs = {
        "dep_rocks": attr.string_list(doc = "Rock names of the rocks this one requires, parallel to dep_spokes."),
        "dep_spokes": attr.string_list(doc = "Repository names of the rocks this one requires."),
        "modules": attr.string_dict(mandatory = True, doc = "Module name to source path inside the archive."),
        "package": attr.string(mandatory = True, doc = "The rock name."),
        "sha256": attr.string(doc = "The archive digest; empty means unpinned and the fetch fails after printing the digest."),
        "strip_prefix": attr.string(doc = "Directory prefix removed from the archive."),
        "unpinned": attr.string(doc = "Why the rock has no usable digest; empty when pinned."),
        "urls": attr.string_list(mandatory = True, doc = "Archive URLs, tried in order."),
    },
)

def declare(hub, lock_text, sidecar_text):
    """Declares the spokes and the hub of one lock; returns the analysis."""
    result = analyze(hub, lock_text, sidecar_text)
    if result.errors:
        fail("luarocks.lock for %s is unusable:\n  %s" % (hub, "\n  ".join(result.errors)))
    for name in sorted(result.packages.keys()):
        package = result.packages[name]
        luarocks_package(
            name = package.spoke,
            dep_rocks = package.deps,
            dep_spokes = [result.packages[dep].spoke for dep in package.deps],
            modules = package.modules,
            package = name,
            sha256 = package.sha256,
            strip_prefix = package.strip_prefix,
            unpinned = package.unpinned,
            urls = package.urls,
        )
    files = {"BUILD.bazel": hub_root_build(result.packages, _RULES_BZL)}
    for name, package in result.packages.items():
        files[name + "/BUILD.bazel"] = hub_package_build(name, package.spoke)
    hub_repo(name = hub, files = files)
    return result
