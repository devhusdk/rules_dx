"""Tests for the dx_cc_toolchains hub."""

load("//cc/private/toolchain:hub.bzl", "CC_TOOLCHAIN_TYPE", "cc_hub", "cc_hub_bootstrap", "cc_hub_bootstrap_entries", "cc_hub_entries", "cc_hub_names")
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

_CC = str(Label("//config:cc_toolchains_on"))

_STAGE0 = str(Label("@llvm//toolchain:bootstrap_stage0_prebuilt_seed"))

_LATER_STAGES = [str(Label("@llvm//toolchain:bootstrap_" + stage)) for stage in ["stage1_from_source", "stage2_lto_and_fdo_instrumented", "stage3_lto_and_fdo_applied"]]

def _stages(entries):
    return sorted({s: True for e in entries for s in e.target_settings if s == _STAGE0 or s in _LATER_STAGES}.keys())

def _first(names, prefix):
    return [i for i, name in enumerate(names) if name.startswith(prefix)][0]

def cc_hub_tests(name, size):
    """Asserts the hub registers the stage0 C and C++ toolchains in order and keeps later LLVM stages unregistered."""
    entries = cc_hub_entries()
    bootstrap = cc_hub_bootstrap_entries()
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
            expect_equal("every toolchain is gated on cc_toolchains_on", [e.name for e in entries if _CC not in e.target_settings], []),
            expect_equal("registered llvm toolchains select stage0 only", _stages(entries), [_STAGE0]),
        ],
    )
    starlark_test(
        name = name + "_bootstrap",
        size = size,
        mode = "load",
        checks = hub_checks(cc_hub_bootstrap(), CC_TOOLCHAIN_TYPE, len(bootstrap)) + [
            expect_equal("the bootstrap package holds stage1 to stage3", _stages(bootstrap), sorted(_LATER_STAGES)),
            expect_equal("bootstrap names are unique", len({e.name: True for e in bootstrap}), len(bootstrap)),
            expect_equal("every bootstrap toolchain is gated on toolchains_on", [e.name for e in bootstrap if str(Label("//config:toolchains_on")) not in e.target_settings], []),
            expect_equal("every bootstrap toolchain is gated on cc_toolchains_on", [e.name for e in bootstrap if _CC not in e.target_settings], []),
        ],
    )
