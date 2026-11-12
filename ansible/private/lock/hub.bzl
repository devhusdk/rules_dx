"""The ansible hub: one repository whose collections are the locked Galaxy archives."""

load("//core:pins.bzl", "lock_problems", "lock_tag")

_NAME_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789_"

_BUILD_HEADER = """\
package(default_visibility = ["//visibility:public"])

"""

def _all_in(text, allowed):
    """Returns True when text is non-empty and only uses allowed characters."""
    if not text:
        return False
    for ch in text.elems():
        if ch not in allowed:
            return False
    return True

def ansible_split_name(name):
    """Splits 'ns.coll' into (ns, coll), or None when malformed."""
    if type(name) != "string":
        return None
    parts = name.split(".")
    if len(parts) != 2 or not _all_in(parts[0], _NAME_CHARS) or not _all_in(parts[1], _NAME_CHARS):
        return None
    return (parts[0], parts[1])

def ansible_manifest_error(package, manifest):
    """Returns why MANIFEST.json does not match the lock package, or None."""
    info = manifest.get("collection_info") if type(manifest) == "dict" else None
    if type(info) != "dict":
        return "%s: MANIFEST.json has no collection_info" % package["name"]
    actual = "%s.%s" % (info.get("namespace"), info.get("name"))
    if actual != package["name"]:
        return "%s: archive contains %s, not the locked collection" % (package["name"], actual)
    if info.get("version") != package["version"]:
        return "%s: archive is version %s but the lock says %s" % (package["name"], info.get("version"), package["version"])
    return None

def ansible_missing_deps(packages, manifests):
    """Returns one message per dependency that no lock package provides."""
    locked = {package["name"]: True for package in packages}
    missing = []
    for package in packages:
        manifest = manifests.get(package["name"])
        info = manifest.get("collection_info", {}) if manifest else {}
        for dep in sorted((info.get("dependencies") or {}).keys()):
            if dep not in locked:
                missing.append("%s depends on %s, which is not in the lock" % (package["name"], dep))
    return missing

def _fail(message):
    fail("ansible collections: " + message)

def _collections_repo_impl(rctx):
    lock = json.decode(rctx.attr.lock)
    packages = lock["packages"]
    manifests = {}
    blocks = [_BUILD_HEADER]
    for package in packages:
        name = package["name"]
        parts = ansible_split_name(name)
        if parts == None:
            _fail("%r is not namespace.collection in lowercase letters, digits and underscores" % name)
        base = "ansible_collections/%s/%s" % parts
        for archive in package.get("archives", []):
            rctx.download_and_extract(
                url = archive["urls"],
                sha256 = archive["sha256"],
                output = base,
                strip_prefix = archive.get("strip_prefix", ""),
                type = archive.get("type", "tar.gz"),
            )
        manifest_path = rctx.path(base + "/MANIFEST.json")
        if not manifest_path.exists:
            _fail("%s: archive has no MANIFEST.json at its root" % name)
        manifest = json.decode(rctx.read(manifest_path))
        mismatch = ansible_manifest_error(package, manifest)
        if mismatch:
            _fail(mismatch)
        manifests[name] = manifest
        blocks.append("""\
filegroup(
    name = "{name}",
    srcs = glob(
        ["{base}/**"],
        exclude = [
            "{base}/tests/**",
            "{base}/.github/**",
        ],
    ),
)

""".format(name = name, base = base))
    missing = ansible_missing_deps(packages, manifests)
    if missing:
        _fail("\n".join(missing))
    blocks.append("""\
filegroup(
    name = "collections",
    srcs = [{srcs}],
)
""".format(srcs = ", ".join(['":%s"' % package["name"] for package in packages])))
    rctx.file("BUILD.bazel", "".join(blocks))
    return rctx.repo_metadata(reproducible = True)

_collections_repo = repository_rule(
    implementation = _collections_repo_impl,
    attrs = {
        "lock": attr.string(mandatory = True, doc = "The decoded dx lock as JSON text."),
    },
    doc = "Lays out the locked collections under ansible_collections/<ns>/<coll>/.",
)

def _ansible_impl(mctx):
    direct = []
    dev = []
    seen = {}
    for module in mctx.modules:
        for tag in module.tags.lock:
            if tag.name in seen:
                _fail("hub %r is declared by both %s and %s" % (tag.name, seen[tag.name], module.name))
            seen[tag.name] = module.name
            text = mctx.read(tag.lock)
            problems = lock_problems(json.decode(text))
            if problems:
                _fail("%s %s" % (tag.lock, "; ".join(problems)))
            _collections_repo(name = tag.name, lock = text)
            if module.is_root:
                if mctx.is_dev_dependency(tag):
                    dev.append(tag.name)
                else:
                    direct.append(tag.name)
    return mctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = dev,
    )

ansible = module_extension(
    doc = "Builds one hub of Galaxy collections per lock tag; check time never reaches Galaxy.",
    implementation = _ansible_impl,
    tag_classes = {"lock": lock_tag},
)
