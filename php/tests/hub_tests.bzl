"""The checks on the composer hub BUILD file."""

load("//php/tests:checks.bzl", "expect_contains", "expect_equal", "php_checks_test")
load("//php/private/lock:hub.bzl", "build_file")

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
        checks = _checks(),
        size = size,
    )
