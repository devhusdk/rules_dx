"""The checks on the composer hub BUILD file."""

load("//core:pins.bzl", "lock_problems")
load("//php/tests:checks.bzl", "expect_contains", "expect_equal", "php_checks_test")
load("//php/private/lock:hub.bzl", "build_file")
load("//php/private/lock:repo.bzl", "composer_problems")

_WITH_DEPS = build_file({
    "deps": ["psr/http-message", "symfony/deprecation-contracts"],
    "extra": {"composer_manifest": {"name": "psr/http-factory", "type": "library"}},
    "name": "psr/http-factory",
    "version": "1.1.0",
})

_NO_DEPS = build_file({
    "extra": {"composer_manifest": {"name": "psr/http-message"}},
    "name": "psr/http-message",
    "version": "2.0",
})

_ARCHIVE = {
    "sha256": "a3c397252b6c1c15e60ad6fae927d61edbf4059f3473080bb1b3b40deabf80d2",
    "strip_prefix": "php-fig-http-factory-2b4765f",
    "type": "zip",
    "urls": ["https://api.github.com/repos/php-fig/http-factory/zipball/2b4765fddfe3b508ac62f829e852b1501d3f6e8a"],
}

def _package(name, deps = [], **overrides):
    package = {
        "archives": [_ARCHIVE],
        "deps": deps,
        "extra": {"composer_manifest": {"name": name}},
        "name": name,
        "version": "1.0.0",
    }
    package.update(overrides)
    return package

def _lock(*packages, **overrides):
    lock = {"ecosystem": "composer", "packages": list(packages), "schema": 1}
    lock.update(overrides)
    return lock

_FACTORY = _package("psr/http-factory", deps = ["psr/http-message"])
_MESSAGE = _package("psr/http-message")

def _problems(*packages, **overrides):
    lock = _lock(*packages, **overrides)
    return lock_problems(lock) or composer_problems(lock)

def _lock_checks():
    return [
        expect_equal("a pinned lock with resolved edges has no problem", _problems(_FACTORY, _MESSAGE), []),
        expect_equal("a package without a composer manifest fails", _problems(_package("psr/http-message", extra = {})), ["package psr/http-message has no composer_manifest object"]),
        expect_equal("another ecosystem fails", _problems(_MESSAGE, ecosystem = "npm"), ["has ecosystem \"npm\", not composer"]),
    ]

def _checks():
    return [
        expect_contains("a package target is the last name segment", _WITH_DEPS, 'php_composer_package(\n    name = "http-factory",'),
        expect_contains("the package name is the composer name", _WITH_DEPS, '    package = "psr/http-factory",\n    version = "1.1.0",'),
        expect_contains("the manifest is carried as JSON", _WITH_DEPS, 'manifest_json = "{\\"name\\":\\"psr/http-factory\\",\\"type\\":\\"library\\"}",'),
        expect_contains("a dependency is a label of the hub", _WITH_DEPS, 'deps = ["//psr/http-message", "//symfony/deprecation-contracts"],'),
        expect_contains("a package without dependencies has an empty list", _NO_DEPS, "    deps = [],"),
        expect_equal("the load is the php rules", _NO_DEPS.split("\n")[0], 'load("%s", "php_composer_package")' % str(Label("//php/private:php.bzl"))),
    ]

def php_hub_tests(name, size):
    """Declares the test that runs the composer hub checks."""
    php_checks_test(
        name = name,
        checks = _checks() + _lock_checks(),
        size = size,
    )
