"""Adds an Ansible collection and its dependencies to a collections lock.json.

Run through the ansible_lock target: bazel run //path:lock -- community.general [version]
It needs network access to Galaxy.
"""

import argparse
import json
import os
import sys
import urllib.request

GALAXY = "https://galaxy.ansible.com/api/v3/plugin/ansible/content/published/collections/index"


def fetch_json(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return json.load(response)


def galaxy_entry(name, version, fetch=fetch_json):
    """Returns (lock entry, dependency names) for one collection from the Galaxy API."""
    namespace, _, collection = name.partition(".")
    base = "%s/%s/%s" % (GALAXY, namespace, collection)
    if not version:
        version = fetch(base + "/")["highest_version"]["version"]
    detail = fetch("%s/versions/%s/" % (base, version))
    entry = {
        "name": name,
        "sha256": detail["artifact"]["sha256"],
        "urls": [detail["download_url"]],
        "version": version,
    }
    return entry, sorted((detail.get("metadata", {}).get("dependencies") or {}).keys())


def resolve(name, version, existing, fetch=fetch_json):
    """Returns the collections for name and its transitive dependencies.

    Dependencies already in the lock keep their entry. New ones use the latest Galaxy version.
    """
    found = {}
    pending = [(name, version)]
    while pending:
        current, wanted = pending.pop()
        if current in found:
            continue
        if current != name and current in existing:
            found[current] = existing[current]
            continue
        entry, deps = galaxy_entry(current, wanted, fetch)
        found[current] = entry
        pending.extend((dep, None) for dep in deps)
    return found


def render(entries):
    """Renders lock.json text: collections sorted by name."""
    ordered = [entries[key] for key in sorted(entries)]
    return json.dumps({"collections": ordered}, indent=2) + "\n"


def update(lock_text, name, version, fetch=fetch_json):
    """Returns lock_text with name and its dependencies added or replaced."""
    existing = {}
    if lock_text.strip():
        existing = {entry["name"]: entry for entry in json.loads(lock_text)["collections"]}
    existing.update(resolve(name, version, existing, fetch))
    return render(existing)


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", required=True, help="lock.json path relative to the workspace")
    parser.add_argument("collection", help="namespace.collection")
    parser.add_argument("version", nargs="?", help="version, default the latest")
    args = parser.parse_args(argv)
    if args.collection.count(".") != 1:
        parser.error("collection must look like namespace.collection")
    path = os.path.join(os.environ.get("BUILD_WORKSPACE_DIRECTORY", "."), args.lock)
    text = ""
    if os.path.exists(path):
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(update(text, args.collection, args.version))
    print("wrote " + path)


if __name__ == "__main__":
    main(sys.argv[1:])
