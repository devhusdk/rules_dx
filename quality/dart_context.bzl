"""Pure helpers that describe the project layout `dart format` and `dart analyze` need.

Records in, strings and paths out, so other consumers can reuse them. A record is any struct with
`package_name`, `lib_root` and optionally `language_version` (a rules_dart `DartPackageInfo`).
The stub pubspec follows rules_dart 0.8.0 `dart/private/project_staging.bzl` `pubspec_stub`, which
is private API and so re-implemented here.
"""

DART_FORMAT = "format"
DART_ANALYZE = "analyze"

def _is_external(lib_root):
    return lib_root.startswith("../")

def dart_external_root(lib_root):
    """Maps an external `../<repo>[/<sub>]` package root onto the shared `_external/` scratch dir."""
    return "_external/" + lib_root[3:]

def dart_package_entries(packages, mode):
    """Returns the `package_config.json` package entries, sorted by root.

    Format mode lists in-repo packages only (the byte-identical layout `dart format` always had).
    Analyze mode adds external packages under `_external/` and fails closed when one package
    name points at two roots or two names share a root.
    """
    roots = {}
    for pkg in packages:
        if _is_external(pkg.lib_root) and mode != DART_ANALYZE:
            continue
        if pkg.lib_root in roots:
            continue
        roots[pkg.lib_root] = pkg
    entries = []
    names = {}
    for lib_root in sorted(roots):
        pkg = roots[lib_root]
        name = pkg.package_name or "dx_package_%d" % len(entries)
        if mode == DART_ANALYZE and name in names:
            fail("dart: package %s is rooted at both '%s' and '%s'" % (name, names[name], lib_root))
        names[name] = lib_root
        root_uri = "../" + (dart_external_root(lib_root) if _is_external(lib_root) else lib_root)
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
    """Returns a valid stub `pubspec.yaml` for the in-repo package `name`.

    Package-root lints read it; every closure package is listed as `any` so that
    `depend_on_referenced_packages` does not blame the staging harness.
    """
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
        if _is_external(pkg.lib_root) or not pkg.package_name:
            continue
        previous = chosen.get(pkg.lib_root)
        if previous == None or pkg.package_name < previous:
            chosen[pkg.lib_root] = pkg.package_name
    stubs = []
    for lib_root in sorted(chosen):
        path = (lib_root + "/pubspec.yaml") if lib_root else "pubspec.yaml"
        stubs.append((path, dart_pubspec_stub(packages, chosen[lib_root], sdk_version)))
    return stubs
