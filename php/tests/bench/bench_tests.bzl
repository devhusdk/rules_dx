"""Load-time checks for php benchmark targets."""

load("//php/tests:checks.bzl", "expect_equal", "php_checks_test")

def php_runner_attr_test(name, runner, expected):
    """Compares attributes of `runner`, declared earlier in this package, with `expected`."""
    rule = native.existing_rule(runner)
    if rule == None:
        fail("php_runner_attr_test: %s is not declared before this call" % runner)
    php_checks_test(
        name = name,
        size = "small",
        checks = [
            expect_equal("%s %s" % (runner, attr), rule.get(attr), value)
            for attr, value in sorted(expected.items())
        ],
    )
