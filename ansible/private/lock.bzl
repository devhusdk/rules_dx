"""Pure validation for the Ansible collections lock."""

_NAME_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789_"
_HEX = "0123456789abcdef"
_ENTRY_KEYS = ["name", "path", "sha256", "urls", "version"]

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

def _entry_error(index, entry):
    """Returns the first problem with one lock entry, or None."""
    if type(entry) != "dict":
        return "collections[%d] must be an object" % index
    for key in entry:
        if key not in _ENTRY_KEYS:
            return "collections[%d] has unknown key %r" % (index, key)
    if ansible_split_name(entry.get("name")) == None:
        return "collections[%d] name %r must look like namespace.collection (lowercase letters, digits, underscore)" % (index, entry.get("name"))
    name = entry["name"]
    if type(entry.get("version")) != "string" or not entry["version"]:
        return "%s: version must be a non-empty string" % name
    sha = entry.get("sha256")
    if type(sha) != "string" or len(sha) != 64 or not _all_in(sha, _HEX):
        return "%s: sha256 must be 64 lowercase hex characters" % name
    has_urls = "urls" in entry
    has_path = "path" in entry
    if has_urls == has_path:
        return "%s: set exactly one of urls or path" % name
    if has_urls:
        urls = entry["urls"]
        if type(urls) != "list" or not urls:
            return "%s: urls must be a non-empty list" % name
        for url in urls:
            if type(url) != "string" or not url:
                return "%s: urls must hold non-empty strings" % name
    elif type(entry["path"]) != "string" or not entry["path"] or entry["path"].startswith("/") or ".." in entry["path"].split("/"):
        return "%s: path must be relative to the lock and must not climb out of it" % name
    return None

def ansible_lock_error(parsed):
    """Returns the first schema problem in a parsed lock, or None when valid."""
    if type(parsed) != "dict":
        return "lock must be a JSON object"
    for key in parsed:
        if key not in ["version", "collections"]:
            return "lock has unknown key %r" % key
    if parsed.get("version") != 1:
        return "unsupported lock version %r: want 1" % parsed.get("version")
    entries = parsed.get("collections")
    if type(entries) != "list":
        return "lock collections must be a list"
    previous = None
    for index, entry in enumerate(entries):
        problem = _entry_error(index, entry)
        if problem:
            return problem
        name = entry["name"]
        if previous != None and name == previous:
            return "duplicate collection %s" % name
        if previous != None and name < previous:
            return "collections must be sorted by name: %s comes after %s" % (name, previous)
        previous = name
    return None

def ansible_manifest_error(entry, manifest):
    """Returns why MANIFEST.json does not match the lock entry, or None."""
    info = manifest.get("collection_info") if type(manifest) == "dict" else None
    if type(info) != "dict":
        return "%s: MANIFEST.json has no collection_info" % entry["name"]
    actual = "%s.%s" % (info.get("namespace"), info.get("name"))
    if actual != entry["name"]:
        return "%s: archive contains %s, not the locked collection" % (entry["name"], actual)
    if info.get("version") != entry["version"]:
        return "%s: archive is version %s but the lock says %s" % (entry["name"], info.get("version"), entry["version"])
    return None

def ansible_missing_deps(entries, manifests):
    """Returns one message per dependency that no lock entry provides."""
    locked = {entry["name"]: True for entry in entries}
    missing = []
    for entry in entries:
        manifest = manifests.get(entry["name"])
        info = manifest.get("collection_info", {}) if manifest else {}
        for dep in sorted((info.get("dependencies") or {}).keys()):
            if dep not in locked:
                missing.append("%s depends on %s, which is not in the lock" % (entry["name"], dep))
    return missing
