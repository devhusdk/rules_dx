"""Stable-stack compose pins."""

load("//modules:versions.bzl", _BAZEL_VERSION = "BAZEL_VERSION", _RULES_CC_VERSION = "RULES_CC_VERSION", _RULES_RUST_VERSION = "RULES_RUST_VERSION", _RUSTFMT_VERSION = "RUSTFMT_VERSION", _RUST_EDITION = "RUST_EDITION", _RUST_VERSION = "RUST_VERSION")

BAZEL_VERSION = _BAZEL_VERSION
RULES_RUST_VERSION = _RULES_RUST_VERSION
RULES_CC_VERSION = _RULES_CC_VERSION
RUST_VERSION = _RUST_VERSION
RUST_EDITION = _RUST_EDITION
RUSTFMT_VERSION = _RUSTFMT_VERSION

STABLE_LOCK_FILE = "//:MODULE.bazel.lock"
STABLE_LOCK_INTEGRITY = "registryFileHashes"

RULES_RS_VERSION = "v0.0.109"
RULES_RS_COMMIT = "b55b132af0c9951807c926768e40222330348632"
PATCHED_RULES_RUST_COMMIT = "e9dd49f22cfa43c75ba30cd9d9bb7d8bdc459dde"
PATCHED_RULES_RUST_MODULE = _RULES_RUST_VERSION
LLVM_BASELINE_MODULE = "0.8.18"
LLVM_BASELINE_LLVM = "22.1.8"
LLVM_TARGET_MODULE = "0.8.19"
LLVM_TARGET_LLVM = "23.1.0"
LLVM_TARGET_COMMIT = "6314688712edf3a95f78642d80393868256b4ef2"
TOOLCHAINS_MSVC_COMMIT = "8e2aa4624bbb5a53a94f135e90995f307875d1ad"

COMPILER_ARCHIVE_INDEX = "extensions/llvm_toolchain_minimal_index.json"
SOURCE_ACQUISITION = "extensions/llvm.bzl"
FREEZE_NOTE = "Freeze compiler archives, runtime sources, upstream patches, SDK manifests and package hashes separately from ruleset source hashes"

BAZEL_BASELINE_NOTE = "Bazel 9 is a provisional initial coverage baseline because the inspected hermetic-llvm coverage fixture requires it, not a release pin"
RELEASE_DEFAULT_NOTE = "the release default follows and the exact seed pin is tracked here"
NO_SILENT_INHERIT = "Do not silently inherit rules_rs's older compiler default or turn a research version into a release pin"
NO_SECOND_GRAPH = "do not compose a second independent Rust rules graph"

STABLE_FIXTURE_RUST_HELLO = "//rust/tests/fixtures/hello:hello"
STABLE_FIXTURE_RUST_TEST = "//rust/tests/fixtures/hello:hello_test"
STABLE_FIXTURE_CC_HELLO = "//cc/tests/fixtures/hello:hello"
STABLE_FIXTURE_CC_TEST = "//cc/tests/fixtures/hello:hello_test"

STABLE_REJECTED = "ad-hoc compose rejected: floating llvm.version override plus MODULE llvm dep plus cxx.rs second graph plus silent older-compiler inherit plus research version as release pin plus host discovery plus bazel_binaries.download second Bazel rejected"
