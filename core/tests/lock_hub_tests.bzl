"""The checks on the shared lock hub."""

load("//core:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//core:pins.bzl", "hub_name", "lock_problems")

_DIGEST = "a" * 64

_PINNED = {"sha256": _DIGEST, "urls": ["https://registry.example/alpha.pkg"]}

def _package(name, deps = [], archive = _PINNED):
    return {"archives": [archive], "deps": deps, "name": name, "version": "1.0.0"}

def _lock(archive, *packages, **groups):
    return {
        "ecosystem": "fixture",
        "groups": groups,
        "inputs": {"fixture.json": _DIGEST},
        "packages": [_package("alpha", archive = archive)] + list(packages),
        "schema": 1,
    }

def _checks():
    pinned = _PINNED
    return [
        expect_equal("a pinned lock has no problem", lock_problems(_lock(pinned)), []),
        expect_true("an archive without a digest fails", len(lock_problems(_lock({"urls": pinned["urls"]}))) == 1),
        expect_true("a short digest fails", len(lock_problems(_lock({"sha256": "abc", "urls": pinned["urls"]}))) == 1),
        expect_true("an empty digest fails", len(lock_problems(_lock({"sha256": "", "urls": pinned["urls"]}))) == 1),
        expect_true("a non hex digest fails", len(lock_problems(_lock({"sha256": "z" * 64, "urls": pinned["urls"]}))) == 1),
        expect_true("a number digest fails", len(lock_problems(_lock({"sha256": 64, "urls": pinned["urls"]}))) == 1),
        expect_true("a list digest fails", len(lock_problems(_lock({"sha256": ["a"] * 64, "urls": pinned["urls"]}))) == 1),
        expect_true("an upper case digest fails", len(lock_problems(_lock({"sha256": "A" * 64, "urls": pinned["urls"]}))) == 1),
        expect_true("a plain http url fails", len(lock_problems(_lock({"sha256": _DIGEST, "urls": ["http://registry.example/a"]}))) == 1),
        expect_true("another schema fails", len(lock_problems(dict(_lock(pinned), schema = 2))) == 1),
        expect_equal("a dep inside the lock resolves", lock_problems(_lock(pinned, _package("beta", deps = ["alpha"]))), []),
        expect_equal("a duplicate name fails", lock_problems(_lock(pinned, _package("alpha"))), ["package alpha appears 2 times"]),
        expect_equal("a dangling dep fails", lock_problems(_lock(pinned, _package("beta", deps = ["gamma"]))), ["package beta depends on gamma, which is not in the lock"]),
        expect_equal("a package that is not an object fails", lock_problems(_lock(pinned, "beta")), ["has a package that is not a JSON object"]),
        expect_equal("a strip_prefix that is not a string fails", lock_problems(_lock(dict(pinned, strip_prefix = 1))), ["package alpha has an archive whose strip_prefix is not a string"]),
        expect_equal("a name may repeat across closures", lock_problems(_lock(pinned, tools = [_package("alpha")])), []),
        expect_equal("a duplicate name in a group fails", lock_problems(_lock(pinned, tools = [_package("beta"), _package("beta")])), ["group tools package beta appears 2 times"]),
        expect_equal("a group dep resolves only inside its group", lock_problems(_lock(pinned, tools = [_package("beta", deps = ["alpha"])])), ["group tools package beta depends on alpha, which is not in the lock"]),
        expect_equal("a non-object package in a group fails", lock_problems(_lock(pinned, tools = [1])), ["group tools has a package that is not a JSON object"]),
        expect_equal("the default hub keeps its name", hub_name("composer", ""), "composer"),
        expect_equal("another set is <hub>_<dir>", hub_name("composer", "tools/web"), "composer_tools.web"),
    ]

def lock_hub_tests(name, size):
    """Declares the test that runs the lock hub checks."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
        size = size,
    )
