"""Pin tests for the OCaml source-built tools."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//ocaml/toolchain:pins.bzl", "OCAML_VERSION")
load(":pins.bzl", "DUNIVERSE", "ZANUDA_OCAML_RANGES", "ZANUDA_VERSION")
load(":repos.bzl", "patch_package")

def _version(text):
    return tuple([int(part) for part in text.split(".")])

def in_ranges(version, ranges):
    """Returns whether version lies in one of the [lo, hi) ranges."""
    return any([_version(lo) <= _version(version) and _version(version) < _version(hi) for lo, hi in ranges])

def _checks():
    dirs = [p.dir for p in DUNIVERSE]
    return [
        expect_true("zanuda accepts the pinned OCaml toolchain", in_ranges(OCAML_VERSION, ZANUDA_OCAML_RANGES)),
        expect_true("zanuda rejects OCaml 5.6", not in_ranges("5.6.0", ZANUDA_OCAML_RANGES)),
        expect_equal("zanuda version", [p.version for p in DUNIVERSE if p.name == "zanuda"], [ZANUDA_VERSION]),
        expect_equal("directories are unique", len(dirs), len(depset(dirs).to_list())),
        expect_true("archive count stays below the abort threshold", len(DUNIVERSE) <= 25),
        expect_true("digests are sha256", all([len(p.sha256) == 64 for p in DUNIVERSE])),
        expect_true("every url is https", all([p.url.startswith("https://") for p in DUNIVERSE])),
        expect_equal("patch names map to directories", patch_package("zanuda.1-no-plugins.patch"), "zanuda"),
        expect_true("dune keeps its bootstrap directory", "dune_src" in dirs),
    ]

def ocaml_tools_tests(name):
    """Declares the pin tests."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
