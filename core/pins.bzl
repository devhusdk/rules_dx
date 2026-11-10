"""Pin helpers shared by the package-hub repository rules."""

def spoke_name(hub, package, replaced, replacement = "_"):
    """Returns the repository name of one package: <hub>__<name>, with each character in `replaced` turned into `replacement`."""
    for char in replaced.elems():
        package = package.replace(char, replacement)
    return hub + "__" + package

def pin_problem(sidecar, field, expected, found, pin):
    """Returns why a package has no usable digest ("" when it is pinned); `field` is the sidecar key recording `expected` and `found` is the label of its source file."""
    if not pin:
        return "has no entry in %s" % sidecar
    if pin.get(field, "") != expected:
        return "is pinned to %s %r but %s has %r; re-pin it" % (field, pin.get(field, ""), found, expected)
    if len(pin.get("sha256", "")) != 64:
        return "has no 64 hex character sha256; re-pin it"
    return ""

def pin_entry_json(name, field, value, sha256):
    """Returns the sidecar entry to commit for one package."""
    return json.encode_indent({name: {field: value, "sha256": sha256}}, indent = "  ")

def download_pinned(rctx, archive, entry):
    """Downloads the archive of a package repository from its `urls` or `url` attribute, failing with the digest to commit when unpinned; `entry` is the sidecar entry to print."""
    attrs = rctx.attr
    kwargs = {"url": getattr(attrs, "urls", None) or [attrs.url], "output": archive}
    if attrs.sha256:
        kwargs["sha256"] = attrs.sha256
    download = rctx.download(**kwargs)
    if attrs.unpinned:
        rctx.delete(archive)
        fail("%s %s\nDownloaded sha256 is %s. Review it and commit:\n%s" % (
            attrs.package,
            attrs.unpinned,
            download.sha256,
            entry(download.sha256),
        ))

def fetch_and_extract(rctx, archive, entry, type = ""):
    """Downloads the archive of a package repository and extracts it with its single top directory stripped, failing like download_pinned when unpinned; `type` is the archive type when the name of `archive` does not tell."""
    download_pinned(rctx, archive, entry)
    rctx.extract(archive, output = "_probe", type = type)
    top = rctx.path("_probe").readdir()
    strip = ""
    if len(top) == 1 and top[0].is_dir:
        strip = top[0].basename
    rctx.delete("_probe")
    rctx.extract(archive, strip_prefix = strip, type = type)
    rctx.delete(archive)

_HEX = "0123456789abcdef"

def hub_name(hub, directory):
    """Returns the hub repository name of the set in `directory`: `hub`, else <hub>_<directory> with `/` written as `.`."""
    if not directory:
        return hub
    return hub + "_" + directory.replace("/", ".")

def lock_problems(lock):
    """Returns why a decoded dx lock cannot build a hub: one message per problem, empty when it can."""
    problems = []
    if type(lock) != "dict":
        return ["is not a JSON object"]
    if lock.get("schema") != 1:
        problems.append("has schema %r, not 1" % lock.get("schema"))
    packages = lock.get("packages")
    if type(packages) != "list":
        return problems + ["has no packages list"]
    for package in packages:
        name = package.get("name", "")
        if not name:
            problems.append("has a package without a name")
            continue
        if not package.get("version"):
            problems.append("package %s has no version" % name)
        for archive in package.get("archives", []):
            digest = archive.get("sha256", "")
            if len(digest) != 64 or any([char not in _HEX for char in digest.elems()]):
                problems.append("package %s has an archive without a 64 hex character sha256" % name)
            urls = archive.get("urls", [])
            if not urls or any([not url.startswith("https://") for url in urls]):
                problems.append("package %s has an archive without https urls" % name)
    return problems

def _lock_repo_impl(rctx):
    lock = json.decode(rctx.attr.lock)
    for package in lock["packages"]:
        name = package["name"]
        for archive in package.get("archives", []):
            rctx.download_and_extract(
                url = archive["urls"],
                sha256 = archive["sha256"],
                output = name,
                strip_prefix = archive.get("strip_prefix", ""),
            )
        rctx.file(name + "/BUILD.bazel", rctx.attr.builds[name])
    rctx.file("BUILD.bazel", "")

lock_repo = repository_rule(
    implementation = _lock_repo_impl,
    attrs = {
        "builds": attr.string_dict(mandatory = True),
        "lock": attr.string(mandatory = True),
    },
)

_lock_tag = tag_class(
    attrs = {
        "lock": attr.label(mandatory = True, allow_single_file = [".json"]),
        "name": attr.string(mandatory = True),
    },
)

def lock_hub(build_file):
    """Returns the module extension of one dx class ecosystem; `build_file(package)` returns the BUILD file text of a decoded lock package."""

    def _impl(mctx):
        for module in mctx.modules:
            for tag in module.tags.lock:
                text = mctx.read(tag.lock)
                lock = json.decode(text)
                problems = lock_problems(lock)
                if problems:
                    fail("%s %s" % (tag.lock, "; ".join(problems)))
                lock_repo(
                    name = tag.name,
                    lock = text,
                    builds = {package["name"]: build_file(package) for package in lock["packages"]},
                )
        return mctx.extension_metadata(reproducible = True)

    return module_extension(
        implementation = _impl,
        tag_classes = {"lock": _lock_tag},
    )
