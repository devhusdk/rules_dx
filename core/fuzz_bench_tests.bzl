"""Checks that attributes set on a fuzz or bench macro land on the target dx runs."""

load("//core/private/testing:defs.bzl", "expect_equal", "starlark_test")

visibility("public")

def dx_runner_attr_test(name, runner, expected, size = "small"):
    """Compares attributes of `runner`, declared earlier in this package, with `expected`."""
    rule = native.existing_rule(runner)
    if rule == None:
        fail("dx_runner_attr_test: %s is not declared before this call" % runner)
    starlark_test(
        name = name,
        size = size,
        mode = "unit",
        checks = [
            expect_equal("%s %s" % (runner, attr), rule.get(attr), value)
            for attr, value in sorted(expected.items())
        ],
    )
