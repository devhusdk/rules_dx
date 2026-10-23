"""Clang-tidy check-only wiring with the target-coupling decision."""

CLANG_TIDY_VERSION = "hermetic-llvm v0.8.23 (LLVM 23.1.0)"

CLANG_TIDY_ARTIFACT = "authoritative hermetic-llvm LLVM tool targets (clang-tidy), no separate acquisition"

CLANG_TIDY_WIRING = "check-only text diagnostics with optional compile-commands context from the authoritative target"
CLANG_TIDY_CHECK = "clang-tidy text diagnostics file:line:col: warning|error: message [check] (exit 0 clean, exit 1 with findings)"
CLANG_TIDY_FIX = "check-only with the provisional sandbox-apply-and-diff fix flow; --fix plus --export-fixes unbound"
CLANG_TIDY_CONFIG_POLICY = "checked-in .clang-tidy required to change policy; upstream built-in default checks without config, --checks=* maxima never enabled"

CC_FIXTURE_HELLO = "//cc/tests/fixtures/hello:hello_test"

CLANG_TIDY_REJECTED = "silent check-only fallback that misses findings rejected; --checks=* maxima rejected; IN_PLACE patching rejected"
