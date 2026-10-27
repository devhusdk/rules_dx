"""Shared cell setting tests."""

load("//libs/starlark:cells.bzl", "cell_cases", "cell_setting", "unknown_cells")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")

_DEFAULT = "//conditions:default"

_PACKAGE = "//libs/starlark/tests"

_PINNED = {"linux_amd64": None, "linux_arm64": None}

def _checks():
    cases = cell_cases(_PACKAGE, _PINNED.keys())
    return [
        expect_equal("one case per cell", sorted([str(key) for key in cases if key != _DEFAULT]), sorted([str(cell_setting(_PACKAGE, cell)) for cell in _PINNED])),
        expect_equal("pinned cells allow the target", [cases[key] for key in cases if key != _DEFAULT], [[] for _ in _PINNED]),
        expect_equal("other cells are incompatible", [str(label) for label in cases[_DEFAULT]], [str(Label("@platforms//:incompatible"))]),
        expect_equal("settings live in the given package", str(cell_setting(_PACKAGE, "linux_arm64")), str(Label("//libs/starlark/tests:cell_linux_arm64"))),
        expect_equal("unknown cells are reported", unknown_cells(_PINNED, ["linux_amd64", "linux_riscv64"]), ["linux_riscv64"]),
        expect_equal("no cells is always incompatible", list(cell_cases(_PACKAGE, []).keys()), [_DEFAULT]),
    ]

def cells_tests(name):
    """Declares the shared cell setting test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
