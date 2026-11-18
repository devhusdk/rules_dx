"""The hex hub: one repository whose packages are the locked Hex releases."""

load("//core:pins.bzl", "lock_problems", "lock_tag")
load("//core/modules:lock_registry.bzl", "LOCK_REGISTRY")

_DEFS = {
    "elixir": str(Label("//elixir/private:defs.bzl")),
    "erlang": str(Label("//erlang/private:defs.bzl")),
}

def build_file(package, lang = "elixir"):
    """Returns the BUILD file text of a decoded lock package built by the `lang` rules."""
    name = package["name"]
    return "\n".join([
        'load("%s", "%s_COMPATIBLE_WITH", "%s_app")' % (_DEFS[lang], lang.upper(), lang),
        "",
        "%s_app(" % lang,
        '    name = "%s",' % name,
        '    app_name = "%s",' % name,
        '    app_version = "%s",' % package["version"],
        "    target_compatible_with = %s_COMPATIBLE_WITH," % lang.upper(),
        '    visibility = ["//visibility:public"],',
        "    deps = %s," % json.encode(["//" + dep for dep in package.get("deps", [])]),
        ")",
        "",
    ])

def _hex_repo_impl(rctx):
    lock = json.decode(rctx.attr.lock)
    for package in lock["packages"]:
        name = package["name"]
        scratch = "_hex_" + name
        for archive in package["archives"]:
            rctx.download_and_extract(
                url = archive["urls"],
                sha256 = archive["sha256"],
                output = scratch,
                type = "tar",
            )
            rctx.extract(scratch + "/contents.tar.gz", output = name)
            rctx.delete(scratch)
        lang = "elixir" if rctx.path(name + "/" + LOCK_REGISTRY["hex"]["manifest"]).exists else "erlang"
        rctx.file(name + "/BUILD.bazel", build_file(package, lang))
    rctx.file("BUILD.bazel", "")
    return rctx.repo_metadata(reproducible = True)

_hex_repo = repository_rule(
    implementation = _hex_repo_impl,
    attrs = {
        "lock": attr.string(mandatory = True, doc = "The decoded dx lock as JSON text."),
    },
)

def _hex_impl(mctx):
    for module in mctx.modules:
        for tag in module.tags.lock:
            text = mctx.read(tag.lock)
            lock = json.decode(text)
            problems = lock_problems(lock)
            if problems:
                fail("%s %s" % (tag.lock, "; ".join(problems)))
            _hex_repo(
                name = tag.name,
                lock = text,
            )
    return mctx.extension_metadata(reproducible = True)

hex = module_extension(
    implementation = _hex_impl,
    tag_classes = {"lock": lock_tag},
)
