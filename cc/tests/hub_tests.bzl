"""Tests for the dx_cc_toolchains hub."""

load("//cc/private/toolchain:hub.bzl", "CC_TOOLCHAIN_TYPE", "cc_hub", "cc_hub_entries", "cc_hub_names")
load("//core:toolchain.bzl", "hub_checks")
load("//core/private/testing:defs.bzl", "expect_equal", "expect_true", "starlark_test")

_FIRST = [
    "gcc_linux_x86_64",
    "gcc_linux_x86_64_to_arm64",
    "gcc_linux_arm64",
    "gcc_linux_arm64_to_x86_64",
    "gcc_unsupported",
    "msvc_windows_x86_64",
    "msvc_windows_x86_64_to_arm64",
    "msvc_windows_arm64",
    "msvc_windows_arm64_to_x86_64",
    "msvc_unsupported",
    "llvm_fuzzer_macos_arm64",
    "llvm_fuzzer_macos_x86_64",
]

def _first(names, prefix):
    return [i for i, name in enumerate(names) if name.startswith(prefix)][0]

def cc_hub_tests(name, size):
    """Asserts the hub holds every C and C++ toolchain, gated, in registration order."""
    entries = cc_hub_entries()
    names = [e.name for e in entries]
    hub_names = cc_hub_names()
    rest = names[len(_FIRST):]
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = hub_checks(cc_hub(), CC_TOOLCHAIN_TYPE, len(entries)) + [
            expect_equal("sorted names are the registration order", sorted(hub_names), hub_names),
            expect_equal("names are unique", len({n: True for n in hub_names}), len(hub_names)),
            expect_equal("hub names end with the toolchain name", [n.partition("_")[2] for n in hub_names], names),
            expect_equal("gcc, msvc and the fuzzer come first, each guard last", names[:len(_FIRST)], _FIRST),
            expect_true("xcode before clang-cl before llvm", _first(rest, "xcode_") < _first(rest, "clang_cl_") and _first(rest, "clang_cl_") < _first(rest, "llvm_")),
            expect_equal("every toolchain is gated", [n for n, e in zip(names, entries) if not e.target_settings], []),
        ],
    )
