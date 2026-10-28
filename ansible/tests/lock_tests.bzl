"""Unit tests for the Ansible collections lock validation."""

load("//ansible/private:lock.bzl", "ansible_lock_error", "ansible_manifest_error", "ansible_missing_deps")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")

_SHA = "0" * 64

def _entry(name, **kwargs):
    base = {"name": name, "version": "1.0.0", "sha256": _SHA, "urls": ["https://example.invalid/x.tar.gz"]}
    base.update(kwargs)
    return base

def _lock(*entries):
    return {"version": 1, "collections": list(entries)}

def _manifest(namespace, name, version, deps = {}):
    return {"collection_info": {"namespace": namespace, "name": name, "version": version, "dependencies": deps}}

def lock_unit_tests(name):
    """Declares the lock validation test."""
    a = _entry("a.one")
    b = _entry("b.two")
    checks = [
        expect_equal("valid", ansible_lock_error(_lock(a, b)), None),
        expect_equal("valid path", ansible_lock_error(_lock({"name": "a.one", "version": "1", "sha256": _SHA, "path": "x.tar.gz"})), None),
        expect_equal("unknown version", ansible_lock_error({"version": 2, "collections": []}), "unsupported lock version 2: want 1"),
        expect_equal("unknown key", ansible_lock_error({"version": 1, "collections": [], "x": 1}), "lock has unknown key \"x\""),
        expect_equal("unknown entry key", ansible_lock_error(_lock(_entry("a.one", extra = 1))), "collections[0] has unknown key \"extra\""),
        expect_equal("bad name", "namespace.collection" in ansible_lock_error(_lock(_entry("A.one"))), True),
        expect_equal("no dot", "namespace.collection" in ansible_lock_error(_lock(_entry("aone"))), True),
        expect_equal("bad sha", "sha256" in ansible_lock_error(_lock(_entry("a.one", sha256 = "abc"))), True),
        expect_equal("both sources", "exactly one" in ansible_lock_error(_lock(_entry("a.one", path = "x"))), True),
        expect_equal("no source", "exactly one" in ansible_lock_error(_lock({"name": "a.one", "version": "1", "sha256": _SHA})), True),
        expect_equal("climbing path", "relative" in ansible_lock_error(_lock({"name": "a.one", "version": "1", "sha256": _SHA, "path": "../x"})), True),
        expect_equal("duplicate", ansible_lock_error(_lock(a, a)), "duplicate collection a.one"),
        expect_equal("unsorted", "sorted" in ansible_lock_error(_lock(b, a)), True),
        expect_equal("manifest ok", ansible_manifest_error(a, _manifest("a", "one", "1.0.0")), None),
        expect_equal("manifest name", "contains x.one" in ansible_manifest_error(a, _manifest("x", "one", "1.0.0")), True),
        expect_equal("manifest version", "version 2.0.0" in ansible_manifest_error(a, _manifest("a", "one", "2.0.0")), True),
        expect_equal("manifest missing info", "no collection_info" in ansible_manifest_error(a, {}), True),
        expect_equal(
            "missing dependency named",
            ansible_missing_deps([a], {"a.one": _manifest("a", "one", "1.0.0", {"c.three": "*"})}),
            ["a.one depends on c.three, which is not in the lock"],
        ),
        expect_equal("dependency present", ansible_missing_deps([a, b], {"a.one": _manifest("a", "one", "1.0.0", {"b.two": "*"})}), []),
    ]
    starlark_test(name = name, mode = "unit", checks = checks)
