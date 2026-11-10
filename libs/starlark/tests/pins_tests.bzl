"""The checks every pins.json shares."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//libs/starlark:defs.bzl", "expect_true", "starlark_test")
load("//libs/starlark:toolchain.bzl", "pin_checks")

def _archive(pin):
    return struct(
        archive = pin["urls"][0].rpartition("/")[2],
        sha256 = pin["sha256"],
        urls = pin["urls"],
    )

def _checks():
    checks = [expect_true("pins are read", len(PINS) > 0)]
    for name, pins in PINS.items():
        cells = {cell: _archive(pin) for cell, pin in pins["cells"].items() if "urls" in pin}
        checks += [
            expect_true(name + " has cells", len(cells) > 0),
            expect_true(name + " binaries are set", all([pin["binary"] != "" for pin in pins["cells"].values()])),
        ] + [
            check
            for check in pin_checks(cells, pins["version"])
        ]
    return checks

def pins_tests(name):
    """Declares the test that runs pin_checks over every pins.json."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
