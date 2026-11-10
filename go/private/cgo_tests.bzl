"""Tests for the cgo toolchain check."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":cgo.bzl", "cgo_toolchain_error")

_MSG_TAIL = " rules_go would build the package without cgo. Select a gcc or clang toolchain with the GNU ABI, or set cgo = False."

def cgo_toolchain_tests(name):
    starlark_test(
        name = name,
        mode = "load",
        checks = [
            expect_equal(
                "msvc-cl is rejected",
                cgo_toolchain_error("msvc-cl", "x86_64-pc-windows-msvc"),
                "cgo needs a GCC/Clang-style driver, but the C++ toolchain is msvc-cl for x86_64-pc-windows-msvc." + _MSG_TAIL,
            ),
            expect_equal(
                "clang-cl is rejected",
                cgo_toolchain_error("clang-cl", "x86_64-pc-windows-msvc"),
                "cgo needs a GCC/Clang-style driver, but the C++ toolchain is clang-cl for x86_64-pc-windows-msvc." + _MSG_TAIL,
            ),
            expect_equal(
                "clang on the MSVC ABI is rejected",
                cgo_toolchain_error("clang", "x86_64-pc-windows-msvc"),
                "Go cgo on Windows needs the MinGW (GNU) ABI, but the C++ toolchain is clang for x86_64-pc-windows-msvc." + _MSG_TAIL,
            ),
            expect_equal("clang on linux", cgo_toolchain_error("clang", "x86_64-unknown-linux-gnu"), ""),
            expect_equal("gcc on linux", cgo_toolchain_error("gcc", "x86_64-linux-gnu"), ""),
            expect_equal("mingw-gcc", cgo_toolchain_error("mingw-gcc", "x86_64-w64-mingw32"), ""),
            expect_equal("unknown compiler", cgo_toolchain_error("icx", "x86_64-unknown-linux-gnu"), ""),
            expect_equal("empty compiler", cgo_toolchain_error("", ""), ""),
        ],
    )
