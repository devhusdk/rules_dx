"""Pin helpers shared by the package-hub repository rules."""

def spoke_name(hub, package, replaced, replacement = "_"):
    """Returns the repository name of one package: <hub>__<name>, with each character in `replaced` turned into `replacement`."""
    for char in replaced.elems():
        package = package.replace(char, replacement)
    return hub + "__" + package

def pin_problem(sidecar, field, expected, found, pin):
    """Returns why a package has no usable digest ("" when it is pinned).

    `field` is the sidecar key that records `expected` and `found` is the label of its source file.
    """
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
    """Downloads the archive of a package repository from its `urls` or `url` attribute.

    Fails with the digest to commit when the package is unpinned. `entry` is the sidecar entry to print.
    """
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
    """Downloads the archive of a package repository and extracts it with its single top directory stripped.

    Fails like download_pinned when the package is unpinned. `type` is the archive type when the name of `archive` does not tell.
    """
    download_pinned(rctx, archive, entry)
    rctx.extract(archive, output = "_probe", type = type)
    top = rctx.path("_probe").readdir()
    strip = ""
    if len(top) == 1 and top[0].is_dir:
        strip = top[0].basename
    rctx.delete("_probe")
    rctx.extract(archive, strip_prefix = strip, type = type)
    rctx.delete(archive)
