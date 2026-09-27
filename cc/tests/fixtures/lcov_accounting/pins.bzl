"""LCOV accounting pins."""

load("//modules:versions.bzl", _BAZEL_VERSION = "BAZEL_VERSION", _RULES_CC_VERSION = "RULES_CC_VERSION", _RULES_RUST_VERSION = "RULES_RUST_VERSION", _RUST_VERSION = "RUST_VERSION")

RUST_ONLY_SHAPE = "Rust-only LCOV via pinned rules_rust llvm-cov"
RUST_ONLY_FIXTURE = "//rust/tests/fixtures/hello:hello_test"
CC_ONLY_SHAPE = "C/C++-only LCOV via pinned Bazel LLVM source coverage"
CC_ONLY_FIXTURE_LIB = "//cc/tests/fixtures/lcov_accounting:accounting"
CC_ONLY_FIXTURE_TEST = "//cc/tests/fixtures/lcov_accounting:accounting_test"
MIXED_DLL_SHAPE = "mixed plus DLL LCOV unioned by authored source"
MIXED_DLL_FIXTURES = [
    "//rust/tests/fixtures/cxx_identity:bridge",
    "//rust/tests/fixtures/cxx_identity:cxx_identity",
    "//cc/tests/fixtures/linux_corpus:corpus_test",
]
MIXED_DLL_NOTE = "Aggregation deduplicates by authored source and unions hits across the cell's tests"
CXXBRIDGE_CMD_LABEL = "@crates//:cxxbridge-cmd"

MISSED_LINE_SHAPE = "missed-line test with uncovered location"
MISSED_LINE_NOTE = "a zero-hit eligible line fails with its location, not just a rate dip"

BAZEL_VERSION = _BAZEL_VERSION
RULES_RUST_VERSION = _RULES_RUST_VERSION
RULES_CC_VERSION = _RULES_CC_VERSION
RUST_VERSION = _RUST_VERSION
LLVM_BASELINE_MODULE = "0.8.18"
LLVM_BASELINE_LLVM = "22.1.8"
LLVM_TARGET_MODULE = "0.8.19"
LLVM_TARGET_LLVM = "23.1.0"
LLVM_TARGET_COMMIT = "6314688712edf3a95f78642d80393868256b4ef2"
RULES_RS_VERSION = "v0.0.109"
PATCHED_RULES_RUST_COMMIT = "e9dd49f22cfa43c75ba30cd9d9bb7d8bdc459dde"
TOOL_PAIRING_NOTE = "Rust/Clang raw-profile compatibility is the upgrade gate; llvm-cov plus llvm-profdata move with LLVM"

NATIVE_IGNORE_LINE = "LCOV_EXCL_LINE"
NATIVE_IGNORE_START = "LCOV_EXCL_START"
NATIVE_IGNORE_STOP = "LCOV_EXCL_STOP"
NATIVE_IGNORE_REASON = "reason:"
NATIVE_IGNORE_ISSUE = "issue:"
NATIVE_IGNORE_NOTE = "valid ignores exclude their executable lines from the denominator with a nearby specific reason plus issue tracking"
DENOMINATOR_NOTE = "Non-ignored eligible sources absent from reports or never executed remain in the denominator"

REJECTED_ALTERNATIVES = [
    "unaccounted lines",
    "ignored collection failures",
    "averaged percentages",
    "cross-cell union",
    "rounding up",
]

LCOV_ACCOUNTING_FIXTURE_LIB = "//cc/tests/fixtures/lcov_accounting:accounting"
LCOV_ACCOUNTING_FIXTURE_TEST = "//cc/tests/fixtures/lcov_accounting:accounting_test"
LCOV_ACCOUNTING_RUST_HELLO = "//rust/tests/fixtures/hello:hello_test"
LCOV_ACCOUNTING_MIXED_CXX = "//rust/tests/fixtures/cxx_identity:bridge"
LCOV_ACCOUNTING_MIXED_RUST = "//rust/tests/fixtures/cxx_identity:cxx_identity"
LCOV_ACCOUNTING_CORPUS = "//cc/tests/fixtures/linux_corpus:corpus_test"
LCOV_ACCOUNTING_FIXTURE_CORPUS = "//cc/tests/fixtures/lcov_accounting:corpus_starlark"
