"""Pure helpers that describe the project layout `dart format` and `dart analyze` need."""

load("//core/modules:lock_registry.bzl", "LOCK_REGISTRY")

visibility(["//core/...", "//dart/..."])

_PUBSPEC = LOCK_REGISTRY["pub"]["manifest"]

DART_FORMAT = "format"
DART_ANALYZE = "analyze"

def _is_external(package_root):
    return package_root.startswith("../")

def dart_external_root(package_root):
    """Maps an external `../<repo>[/<sub>]` package root onto the shared `_external/` scratch dir."""
    return "_external/" + package_root[3:]

def dart_merge_packages(packages, extra):
    """Returns packages plus the extra ones whose root is not already listed."""
    known = {pkg.package_root: True for pkg in packages}
    return packages + [pkg for pkg in extra if pkg.package_root not in known]

def dart_package_entries(packages, mode):
    """Returns the `package_config.json` package entries, sorted by root."""
    roots = {}
    for pkg in packages:
        if _is_external(pkg.package_root) and mode != DART_ANALYZE:
            continue
        if pkg.package_root in roots:
            continue
        roots[pkg.package_root] = pkg
    entries = []
    names = {}
    for package_root in sorted(roots):
        pkg = roots[package_root]
        name = pkg.package_name or "dx_package_%d" % len(entries)
        if mode == DART_ANALYZE and name in names:
            fail("dart: package %s is rooted at both '%s' and '%s'" % (name, names[name], package_root))
        names[name] = package_root
        root_uri = "../" + (dart_external_root(package_root) if _is_external(package_root) else package_root)
        entry = {
            "name": name,
            "packageUri": "lib/",
            "rootUri": root_uri,
        }
        version = getattr(pkg, "language_version", "")
        if version:
            entry["languageVersion"] = version
        entries.append(entry)
    return entries

def dart_pubspec_stub(packages, name, sdk_version = ""):
    """Returns a valid stub pubspec for the in-repo package `name`."""
    sdk = ">=3.0.0 <4.0.0"
    parts = sdk_version.split(".")
    if len(parts) >= 2:
        sdk = "^%s.%s.0" % (parts[0], parts[1])
    for pkg in packages:
        if pkg.package_name == name and getattr(pkg, "language_version", ""):
            sdk = "^%s.0" % pkg.language_version
            break
    lines = ["name: %s" % name, "environment:", '  sdk: "%s"' % sdk]
    deps = sorted({pkg.package_name: True for pkg in packages if pkg.package_name and pkg.package_name != name})
    if deps:
        lines.append("dependencies:")
        lines.extend(["  %s: any" % dep for dep in deps])
    return "\n".join(lines) + "\n"

def dart_pubspec_stubs(packages, sdk_version = ""):
    """Returns sorted `(scratch_path, content)` stubs, one per in-repo package root."""
    chosen = {}
    for pkg in packages:
        if _is_external(pkg.package_root) or not pkg.package_name:
            continue
        previous = chosen.get(pkg.package_root)
        if previous == None or pkg.package_name < previous:
            chosen[pkg.package_root] = pkg.package_name
    stubs = []
    for package_root in sorted(chosen):
        path = (package_root + "/" + _PUBSPEC) if package_root else _PUBSPEC
        stubs.append((path, dart_pubspec_stub(packages, chosen[package_root], sdk_version)))
    return stubs
