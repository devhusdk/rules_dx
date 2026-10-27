"""Pure composer.lock logic: validation, dependency edges, digests and staged manifests.

Nothing here runs Composer or PHP. Gazelle and a future pin tool can reuse the same facts.
"""

SIDECAR_VERSION = 1

_ARCHIVE_TYPES = ["zip", "tar"]

_PLATFORM_PREFIXES = ("ext-", "lib-", "composer-")

_PLATFORM_NAMES = ["php", "php-64bit", "php-ipv6", "php-zts", "php-debug", "hhvm"]

# Fields of a lock entry that Composer's dump-autoload reads or the ext-* check needs.
_MANIFEST_FIELDS = ["name", "version", "version_normalized", "type", "autoload", "require", "replace", "provide"]

def is_platform(requirement):
    """Returns whether a require key names the platform (php, ext-*, lib-*) rather than a package."""
    return requirement in _PLATFORM_NAMES or requirement.startswith(_PLATFORM_PREFIXES)

def spoke_name(hub, package):
    """Returns the repository name of one package: <hub>__<vendor>__<name>."""
    return hub + "__" + package.replace("/", "__")

def archive_type(dist_type, url):
    """Returns the download_and_extract type of one dist."""
    if dist_type == "zip":
        return "zip"
    if url.endswith((".tgz", ".tar.gz")):
        return "tar.gz"
    return "tar"

def expand_mirror(mirror_url, package, version, reference, dist_type):
    """Expands the %package%, %version%, %reference% and %type% placeholders of a Composer dist mirror."""
    return (mirror_url
        .replace("%package%", package)
        .replace("%version%", version)
        .replace("%reference%", reference)
        .replace("%type%", dist_type))

def package_urls(entry, pin):
    """Returns the download URLs of a lock entry: sidecar mirrors, then lock mirrors, then dist.url."""
    dist = entry["dist"]
    reference = dist.get("reference", "")
    urls = list(pin.get("urls", [])) if pin else []
    for mirror in dist.get("mirrors", []):
        urls.append(expand_mirror(mirror["url"], entry["name"], entry["version"], reference, dist["type"]))
    urls.append(dist["url"])
    out = []
    for url in urls:
        if url not in out:
            out.append(url)
    return out

def parse_sidecar(text):
    """Parses composer.digests.json; returns (packages dict, errors)."""
    if not text.strip():
        return {}, []
    data = json.decode(text)
    if type(data) != "dict" or data.get("version") != SIDECAR_VERSION or type(data.get("packages")) != "dict":
        return {}, ["composer.digests.json must be {\"version\": 1, \"packages\": {...}}"]
    return data["packages"], []

def manifest_entry(entry):
    """Returns the filtered lock entry written to the staged installed.json."""
    out = {key: entry[key] for key in _MANIFEST_FIELDS if entry.get(key)}
    out["type"] = out.get("type", "library")
    return out

def pin_problem(entry, pin):
    """Returns why a package has no usable digest ("" when it is pinned)."""
    reference = entry.get("dist", {}).get("reference", "")
    if not pin:
        return "has no entry in composer.digests.json"
    if pin.get("reference", "") != reference:
        return "is pinned to reference %r but composer.lock has %r; re-pin it" % (pin.get("reference", ""), reference)
    if len(pin.get("sha256", "")) != 64:
        return "has no 64 hex character sha256; re-pin it"
    return ""

def analyze(hub, lock_text, sidecar_text):
    """Validates one lock and sidecar.

    Returns a struct with `errors` (list of strings; empty when usable) and `packages`,
    a dict keyed by package name with: name, version, dev, spoke, urls, archive_type,
    sha256, reference, unpinned (problem text or ""), deps (sorted package names)
    and manifest (dict).
    """
    errors = []
    lock = json.decode(lock_text)
    pins, sidecar_errors = parse_sidecar(sidecar_text)
    errors += sidecar_errors
    entries = []
    for key, dev in (("packages", False), ("packages-dev", True)):
        for entry in lock.get(key) or []:
            entries.append((entry, dev))
    packages = {}
    provided_by = {}
    spokes = {}
    metas = {}
    for entry, dev in entries:
        name = entry["name"]
        kind = entry.get("type", "library")
        if kind == "metapackage":
            # Metapackages ship no files; they only forward their requirements.
            metas[name] = entry
            provided_by[name] = name
            continue
        dist = entry.get("dist") or {}
        if dist.get("type") not in _ARCHIVE_TYPES or not dist.get("url"):
            errors.append("%s has no zip or tar dist (dist type %r); source-only packages are not supported" % (name, dist.get("type")))
            continue
        spoke = spoke_name(hub, name)
        if spoke in spokes:
            errors.append("%s and %s both map to the repository %s" % (spokes[spoke], name, spoke))
            continue
        spokes[spoke] = name
        pin = pins.get(name)
        problem = pin_problem(entry, pin)
        urls = package_urls(entry, pin)
        packages[name] = struct(
            name = name,
            version = entry["version"],
            dev = dev,
            spoke = spoke,
            urls = urls,
            archive_type = archive_type(dist["type"], urls[-1]),
            sha256 = "" if problem else pin["sha256"],
            reference = dist.get("reference", ""),
            unpinned = problem,
            entry = entry,
            deps = [],
            manifest = manifest_entry(entry),
        )
        provided_by[name] = name
    carriers = {name: packages[name].entry for name in packages}
    carriers.update(metas)
    for name in sorted(carriers.keys()):
        for virtual in list((carriers[name].get("replace") or {}).keys()) + list((carriers[name].get("provide") or {}).keys()):
            if virtual not in provided_by:
                provided_by[virtual] = name
    resolved = {}
    for name in sorted(carriers.keys()):
        # Requirements of a metapackage are flattened into whoever requires it.
        pending = [(name, requirement) for requirement in sorted((carriers[name].get("require") or {}).keys())]
        seen = {name: True}
        edges = []
        for _ in range(100000):
            if not pending:
                break
            owner, requirement = pending.pop(0)
            if is_platform(requirement):
                continue
            target = provided_by.get(requirement)
            if target == None:
                errors.append("%s requires %s, which is not in composer.lock" % (owner, requirement))
            elif target in metas:
                if target not in seen:
                    seen[target] = True
                    pending += [(target, r) for r in sorted((metas[target].get("require") or {}).keys())]
            elif target != name and target not in edges:
                edges.append(target)
        if name in metas:
            continue
        resolved[name] = struct(
            name = name,
            version = packages[name].version,
            dev = packages[name].dev,
            spoke = packages[name].spoke,
            urls = packages[name].urls,
            archive_type = packages[name].archive_type,
            sha256 = packages[name].sha256,
            reference = packages[name].reference,
            unpinned = packages[name].unpinned,
            deps = sorted(edges),
            manifest = packages[name].manifest,
        )
    for pin_name in sorted(pins.keys()):
        if pin_name not in packages:
            errors.append("composer.digests.json pins %s, which is not in composer.lock; remove it" % pin_name)
    return struct(errors = errors, packages = resolved)

def pin_entry_json(name, reference, sha256, urls = None):
    """Returns the corrected sidecar entry to commit for one package."""
    entry = {"reference": reference, "sha256": sha256}
    if urls:
        entry["urls"] = urls
    return json.encode_indent({name: entry}, indent = "  ")

def spoke_build(package, package_bzl, dep_spokes):
    """Returns the BUILD file of one package repository."""
    return """load("{bzl}", "php_composer_package")

package(default_visibility = ["//visibility:public"])

php_composer_package(
    name = "pkg",
    srcs = glob(
        ["**"],
        exclude = ["BUILD.bazel", "REPO.bazel", "MODULE.bazel", "WORKSPACE", "WORKSPACE.bazel", "manifest.json"],
    ),
    manifest = "manifest.json",
    package = {name},
    version = {version},
    deps = {deps},
)
""".format(
        bzl = package_bzl,
        name = repr(package.name),
        version = repr(package.version),
        deps = repr(["@%s//:pkg" % spoke for spoke in dep_spokes]),
    )

def hub_package_build(package, spoke):
    """Returns the BUILD file the hub holds for one package; the target is named like the package."""
    return """package(default_visibility = ["//visibility:public"])

alias(
    name = "{short}",
    actual = "@{spoke}//:pkg",
)
""".format(short = package.split("/")[-1], spoke = spoke)

def hub_root_build(packages, groups_bzl):
    """Returns the hub's root BUILD file with :all_packages and :dev_packages."""
    prod = ["//%s" % name for name in sorted(packages.keys()) if not packages[name].dev]
    dev = ["//%s" % name for name in sorted(packages.keys()) if packages[name].dev]
    return """load("{bzl}", "php_composer_group")

package(default_visibility = ["//visibility:public"])

php_composer_group(
    name = "all_packages",
    deps = {prod},
)

php_composer_group(
    name = "dev_packages",
    deps = {dev},
)
""".format(bzl = groups_bzl, prod = repr(prod), dev = repr(dev))
