"""The checks every pins.json shares."""

load("@dx_pins_data//:pins.bzl", "DATA", "PINS")
load("//core:defs.bzl", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "pin_checks")

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
        ]
        for cell, pin in pins["cells"].items():
            if "ghcr_scope" in pin:
                checks += pin_checks({cell: cells[cell]}, "sha256:" + pin["sha256"])
        checks += pin_checks({c: v for c, v in cells.items() if "ghcr_scope" not in pins["cells"][c]}, pins["version"])
    for name, pins in DATA.items():
        entries = pins.get("cells", {}).values() + pins.get("archives", {}).values() + pins.get("hex", {}).values()
        archives = [entry for entry in entries if "urls" in entry]
        checks += [
            expect_true(name + " has entries", len(entries) > 0),
            expect_true(name + " digests are sha256", all([len(entry["sha256"]) == 64 for entry in entries])),
            expect_true(name + " urls are https", all([all([url.startswith("https://") for url in entry["urls"]]) for entry in archives])),
            expect_true(name + " cells are host cells", all([cell in ["linux_arm64", "linux_x86_64", "macos_arm64", "macos_x86_64", "windows_arm64", "windows_x86_64"] for cell in pins.get("cells", {})])),
        ]
    for name, pins in DATA.items():
        entries = pins.get("cells", {}).values() + pins.get("archives", {}).values()
        archives = [entry for entry in entries if "urls" in entry]
        checks += [
            expect_true(name + " has entries", len(entries) > 0),
            expect_true(name + " digests are sha256", all([len(entry["sha256"]) == 64 for entry in entries])),
            expect_true(name + " urls are https", all([all([url.startswith("https://") for url in entry["urls"]]) for entry in archives])),
        ]
    return checks

def pins_tests(name):
    """Declares the test that runs pin_checks over every pins.json."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
