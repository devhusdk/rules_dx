"""OTP cell setting tests."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":cells.bzl", "cell_cases", "cell_setting", "unknown_cells")
load(":pins.bzl", "CELLS")

_DEFAULT = "//conditions:default"

def _checks():
    cases = cell_cases(CELLS.keys())
    return [
        expect_equal("one case per cell", sorted([str(key) for key in cases if key != _DEFAULT]), sorted([str(cell_setting(cell)) for cell in CELLS])),
        expect_equal("pinned cells allow the target", [cases[key] for key in cases if key != _DEFAULT], [[] for _ in CELLS]),
        expect_equal("other cells are incompatible", [str(label) for label in cases[_DEFAULT]], [str(Label("@platforms//:incompatible"))]),
        expect_equal("settings live in the toolchain package", str(cell_setting("linux_arm64")), str(Label("//erlang/toolchain:cell_linux_arm64"))),
        expect_equal("unknown cells are reported", unknown_cells(["linux_amd64", "linux_riscv64"]), ["linux_riscv64"]),
        expect_equal("no cells is always incompatible", list(cell_cases([]).keys()), [_DEFAULT]),
    ]

def erlang_cell_tests(name):
    """Declares the OTP cell setting test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
