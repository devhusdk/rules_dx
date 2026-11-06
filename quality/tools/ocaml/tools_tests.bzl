"""Pin tests for the OCaml source-built tools."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load("//ocaml/toolchain:pins.bzl", "OCAML_VERSION")
load("//quality/artifacts:zanuda.bzl", "SOURCE", "ZANUDA_OCAML_RANGES")
load(":repos.bzl", "patch_package")

def _version(text):
    return tuple([int(part) for part in text.split(".")])

def in_ranges(version, ranges):
    """Returns whether version lies in one of the [lo, hi) ranges."""
    return any([_version(lo) <= _version(version) and _version(version) < _version(hi) for lo, hi in ranges])

def _checks():
    packages = SOURCE["packages"]
    dirs = [p["dir"] for p in packages]
    return [
        expect_true("zanuda accepts the pinned OCaml toolchain", in_ranges(OCAML_VERSION, ZANUDA_OCAML_RANGES)),
        expect_true("zanuda rejects OCaml 5.6", not in_ranges("5.6.0", ZANUDA_OCAML_RANGES)),
        expect_equal("zanuda version", [p["version"] for p in packages if p["name"] == "zanuda"], [SOURCE["version"]]),
        expect_equal("directories are unique", len(dirs), len(depset(dirs).to_list())),
        expect_true("archive count stays below the abort threshold", len(packages) <= 25),
        expect_equal("patch names map to directories", patch_package("zanuda.1-no-plugins.patch"), "zanuda"),
        expect_true("dune keeps its library directory", "dune_src" in dirs),
        expect_true("patches name packages", all([patch_package(p.split("/")[-1]) in dirs for p in SOURCE["patches"]])),
    ]

def ocaml_tools_tests(name):
    """Declares the pin tests."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
