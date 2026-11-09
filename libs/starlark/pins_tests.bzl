"""The checks every pins.json shares."""

load("@dx_pins_data//:pins.bzl", "PINS")
load(":defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":toolchain.bzl", "pin_checks")

def _archive(pin):
    return struct(
        archive = pin["urls"][0].rpartition("/")[2],
        sha256 = pin["sha256"],
        urls = pin["urls"],
    )

def _archive_checks(name, archives):
    checks = [expect_true(name + " has archives", len(archives) > 0)]
    for key, pin in archives.items():
        checks += [
            expect_equal(name + " " + key + " sha256 length", len(pin["sha256"]), 64),
            expect_true(name + " " + key + " url names its version", pin["version"] in pin["urls"][0]),
            expect_true(name + " " + key + " strip_prefix names its version", pin.get("strip_prefix", pin["version"]).count(pin["version"]) > 0),
        ]
    return checks

def _checks():
    checks = [expect_true("pins are read", len(PINS) > 0)]
    for name, pins in PINS.items():
        if "archives" in pins:
            checks += _archive_checks(name, pins["archives"])
            continue
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
