"""Checks that attributes set on a fuzz or bench macro land on the target dx runs."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")

def dx_runner_attr_test(name, runner, expected):
    """Compares attributes of `runner`, declared earlier in this package, with `expected`."""
    rule = native.existing_rule(runner)
    if rule == None:
        fail("dx_runner_attr_test: %s is not declared before this call" % runner)
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal("%s %s" % (runner, attr), rule.get(attr), value)
            for attr, value in sorted(expected.items())
        ],
    )
