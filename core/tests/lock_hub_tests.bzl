"""The checks on the shared lock hub."""

load("//core:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//core:pins.bzl", "hub_name", "lock_problems")

_DIGEST = "a" * 64

def _lock(archive):
    return {
        "ecosystem": "fixture",
        "inputs": {"fixture.json": _DIGEST},
        "packages": [{"archives": [archive], "name": "alpha", "version": "1.0.0"}],
        "schema": 1,
    }

def _checks():
    pinned = {"sha256": _DIGEST, "urls": ["https://registry.example/alpha.pkg"]}
    return [
        expect_equal("a pinned lock has no problem", lock_problems(_lock(pinned)), []),
        expect_true("an archive without a digest fails", len(lock_problems(_lock({"urls": pinned["urls"]}))) == 1),
        expect_true("a short digest fails", len(lock_problems(_lock({"sha256": "abc", "urls": pinned["urls"]}))) == 1),
        expect_true("an upper case digest fails", len(lock_problems(_lock({"sha256": "A" * 64, "urls": pinned["urls"]}))) == 1),
        expect_true("a plain http url fails", len(lock_problems(_lock({"sha256": _DIGEST, "urls": ["http://registry.example/a"]}))) == 1),
        expect_true("another schema fails", len(lock_problems(dict(_lock(pinned), schema = 2))) == 1),
        expect_equal("the default hub keeps its name", hub_name("composer", ""), "composer"),
        expect_equal("another set is <hub>_<dir>", hub_name("composer", "tools/web"), "composer_tools.web"),
    ]

def lock_hub_tests(name):
    """Declares the test that runs the lock hub checks."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
