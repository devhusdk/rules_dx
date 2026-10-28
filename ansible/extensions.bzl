"""Module extension that fetches locked Ansible collections into a hub repository."""

load("//ansible/private:lock.bzl", "ansible_lock_error", "ansible_manifest_error", "ansible_missing_deps", "ansible_split_name")

_BUILD_HEADER = """\
package(default_visibility = ["//visibility:public"])

"""

def _fail(message):
    """Fails with an actionable collections message."""
    fail("ansible collections: " + message)

def _urls(lock_path, entry):
    """Returns the fetch URLs for one lock entry."""
    if "urls" in entry:
        return entry["urls"]
    return ["file://" + str(lock_path.dirname.get_child(entry["path"]))]

def _collections_repo_impl(rctx):
    """Downloads, verifies and lays out every locked collection."""
    lock_path = rctx.path(rctx.attr.lock)
    rctx.watch(lock_path)
    parsed = json.decode(rctx.read(lock_path))
    problem = ansible_lock_error(parsed)
    if problem:
        _fail("%s: %s" % (rctx.attr.lock, problem))
    entries = parsed["collections"]
    manifests = {}
    blocks = [_BUILD_HEADER]
    names = []
    for entry in entries:
        namespace, collection = ansible_split_name(entry["name"])
        base = "ansible_collections/%s/%s" % (namespace, collection)
        if "path" in entry:
            rctx.watch(lock_path.dirname.get_child(entry["path"]))
        rctx.download_and_extract(
            url = _urls(lock_path, entry),
            output = base,
            sha256 = entry["sha256"],
            type = "tar.gz",
        )
        manifest_path = rctx.path(base + "/MANIFEST.json")
        if not manifest_path.exists:
            _fail("%s: archive has no MANIFEST.json at its root" % entry["name"])
        manifest = json.decode(rctx.read(manifest_path))
        mismatch = ansible_manifest_error(entry, manifest)
        if mismatch:
            _fail(mismatch)
        manifests[entry["name"]] = manifest
        names.append(entry["name"])
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

""".format(name = entry["name"], base = base))
    missing = ansible_missing_deps(entries, manifests)
    if missing:
        _fail("\n".join(missing))
    blocks.append("""\
filegroup(
    name = "collections",
    srcs = [{srcs}],
)
""".format(srcs = ", ".join(['":%s"' % name for name in names])))
    rctx.file("BUILD.bazel", "".join(blocks))

_collections_repo = repository_rule(
    implementation = _collections_repo_impl,
    attrs = {
        "lock": attr.label(allow_single_file = [".json"], mandatory = True),
    },
    doc = "Fetches the collections named by an Ansible lock into ansible_collections/<ns>/<coll>/.",
)

_collections = tag_class(
    doc = "One hub of locked Ansible collections.",
    attrs = {
        "lock": attr.label(
            doc = "JSON lock: {version: 1, collections: [{name, version, sha256, urls | path}]}, sorted by name.",
            allow_single_file = [".json"],
            mandatory = True,
        ),
        "name": attr.string(
            doc = "The hub repository name.",
            mandatory = True,
        ),
    },
)

def _ansible_impl(module_ctx):
    """Creates one hub per collections tag and reports the root module's hubs."""
    direct = []
    dev = []
    seen = {}
    for mod in module_ctx.modules:
        for tag in mod.tags.collections:
            if tag.name in seen:
                _fail("hub %r is declared by both %s and %s" % (tag.name, seen[tag.name], mod.name))
            seen[tag.name] = mod.name
            _collections_repo(name = tag.name, lock = tag.lock)
            if mod.is_root:
                if module_ctx.is_dev_dependency(tag):
                    dev.append(tag.name)
                else:
                    direct.append(tag.name)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = dev,
    )

ansible = module_extension(
    doc = "Fetches sha256-pinned Ansible collections; check time never reaches Galaxy.",
    implementation = _ansible_impl,
    tag_classes = {"collections": _collections},
)
