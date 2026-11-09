"""Shared cell setting tests."""

load("//d/toolchain:pins.bzl", "LDC_CELLS")
load("//erlang/toolchain:pins.bzl", erlang_cells = "CELLS")
load("//fortran/toolchain:pins.bzl", "FORTRAN_CELLS")
load("//haskell/toolchain:pins.bzl", haskell_cells = "CELLS")
load("//libs/starlark:cells.bzl", "cell_cases", "cell_setting", "unknown_cells", "unsupported_constraint")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//lua/toolchain:pins.bzl", "LUALS_CELLS")
load("//modules:cuda.bzl", "CUDA_CELLS")
load("//ocaml/toolchain:pins.bzl", "OCAML_CELLS")
load("//php/toolchain:pins.bzl", "PHP_CELLS")
load("//powershell/toolchain:pins.bzl", "PWSH_CELLS")
load("//qt/sdk:pins.bzl", qt_cells = "CELLS")
load("//ruby/toolchain:pins.bzl", "SORBET_CELLS")

_DEFAULT = "//conditions:default"

_PACKAGE = "//libs/starlark/tests"

_PINNED = {"linux_x86_64": None, "linux_arm64": None}

_ALL_CELLS = {
    "cuda": CUDA_CELLS,
    "d": LDC_CELLS,
    "erlang": erlang_cells,
    "fortran": FORTRAN_CELLS,
    "haskell": haskell_cells,
    "lua": LUALS_CELLS,
    "ocaml": OCAML_CELLS,
    "php": PHP_CELLS,
    "powershell": PWSH_CELLS,
    "qt": qt_cells,
    "ruby": SORBET_CELLS,
}

_OSES = ["linux", "macos", "windows"]

_CPUS = ["x86_64", "arm64"]

def _noncanonical(cells):
    return sorted([
        "{}:{}".format(language, key)
        for language, pinned in cells.items()
        for key in pinned
        if key.partition("_")[0] not in _OSES or key.partition("_")[2] not in _CPUS
    ])

def _checks():
    cases = cell_cases(_PACKAGE, _PINNED.keys())
    return [
        expect_equal("one case per cell", sorted([str(key) for key in cases if key != _DEFAULT]), sorted([str(cell_setting(_PACKAGE, cell)) for cell in _PINNED])),
        expect_equal("pinned cells allow the target", [cases[key] for key in cases if key != _DEFAULT], [[] for _ in _PINNED]),
        expect_equal("other cells are incompatible", [str(label) for label in cases[_DEFAULT]], [str(Label("@platforms//:incompatible"))]),
        expect_equal("unsupported default names the language", [str(label) for label in cell_cases(_PACKAGE, _PINNED.keys(), True)[_DEFAULT]], [str(Label("@platforms//:incompatible")), str(unsupported_constraint(_PACKAGE))]),
        expect_equal("settings live in the given package", str(cell_setting(_PACKAGE, "linux_arm64")), str(Label("//libs/starlark/tests:cell_linux_arm64"))),
        expect_equal("unknown cells are reported", unknown_cells(_PINNED, ["linux_x86_64", "linux_riscv64"]), ["linux_riscv64"]),
        expect_equal("a setting replaces the cell setting", sorted([str(key) for key in cell_cases(_PACKAGE, _PINNED.keys(), False, {"linux_arm64": Label("//libs/starlark/tests:other")}) if key != _DEFAULT]), sorted([str(Label("//libs/starlark/tests:cell_linux_x86_64")), str(Label("//libs/starlark/tests:other"))])),
        expect_equal("every toolchain cell key is os_cpu with the canonical words", _noncanonical(_ALL_CELLS), []),
        expect_equal("the key check rejects vendor spellings", _noncanonical({"x": {"linux_amd64": 1, "osx_arm64": 1, "linux_x64": 1, "macos_arm64": 1}}), ["x:linux_amd64", "x:linux_x64", "x:osx_arm64"]),
        expect_equal("no cells is always incompatible", list(cell_cases(_PACKAGE, []).keys()), [_DEFAULT]),
    ]

def cells_tests(name):
    """Declares the shared cell setting test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
