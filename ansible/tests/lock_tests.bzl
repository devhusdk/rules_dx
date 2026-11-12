"""Unit tests for the ansible hub checks of the archives."""

load("//ansible/private/lock:hub.bzl", "ansible_manifest_error", "ansible_missing_deps", "ansible_split_name")
load("//core:defs.bzl", "expect_equal", "starlark_test")

def _entry(name):
    return {"name": name, "version": "1.0.0"}

def _manifest(namespace, name, version, deps = {}):
    return {"collection_info": {"namespace": namespace, "name": name, "version": version, "dependencies": deps}}

def lock_unit_tests(name):
    """Declares the hub check test."""
    a = _entry("a.one")
    b = _entry("b.two")
    checks = [
        expect_equal("split", ansible_split_name("a.one"), ("a", "one")),
        expect_equal("split upper", ansible_split_name("A.one"), None),
        expect_equal("split no dot", ansible_split_name("aone"), None),
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
