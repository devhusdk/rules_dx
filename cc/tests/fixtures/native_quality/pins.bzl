"""Native quality defaults pins."""

load("//modules:versions.bzl", _GO_SDK_VERSION = "GO_SDK_VERSION", _RULES_GO_VERSION = "RULES_GO_VERSION")

CLANG_TOOLCHAIN_VERSION = "hermetic-llvm v0.8.19"
LLVM_VERSION = "23.1.0"
CPPCHECK_VERSION = "2.21.0"
GOFUMPT_VERSION = "v0.11.0"
STATICCHECK_VERSION = "2026.2"
GOVET_TOOLCHAIN_VERSION = _GO_SDK_VERSION
ERRCHECK_VERSION = "v1.20.0"

CLANG_TOOLCHAIN_ROUTE = "authoritative hermetic-llvm LLVM tool targets (clang-format, clang-tidy), no separate acquisition"
CPPCHECK_ARTIFACT = "standalone checksummed release artifact; --xml --xml-version=2 on stderr"
GOFUMPT_ARTIFACT = "standalone checksummed release artifact; strict superset of gofmt"
STATICCHECK_ARTIFACT = "standalone checksummed release artifact; SARIF via -f sarif, JSON via -f json"
GOVET_COUPLING = "ships with the qualified Go toolchain (rules_go " + _RULES_GO_VERSION + " plus Go SDK " + _GO_SDK_VERSION + "), no separate acquisition"
ERRCHECK_ARTIFACT = "standalone checksummed release artifact; complementary for unhandled errors"

REJECTED_LLVM_PRIOR = "hermetic-llvm v0.8.18 plus LLVM 22.1.8 prior line rejected; living at head rejected"
REJECTED_GOVET_OBSERVED = "Go 1.27.1 observed, not pinned; govet follows the qualified Go SDK " + _GO_SDK_VERSION + " pin"
REJECTED_STATICCHECK_BINARY = "v0.8.1 observed binary identity, not the pinned release line; unpinned versions rejected"

NATIVE_CONFIG_POLICY = "native-configuration sole policy: no hidden presets"

STATICCHECK_POLICY = "default checks are the upstream built-in default checks, not an SA-only preset; SA-only shortcut rejected without qualification"
GOVET_POLICY = "default analyzers are the upstream built-in default analyzers; all-analyzer and vettool maxima never enabled"
ERRCHECK_POLICY = "complementary for unhandled errors, not a default selection; check-only"
CLANG_TIDY_POLICY = "default checks are the upstream built-in default checks; --checks=* maxima never enabled"
CPPCHECK_POLICY = "default enablement is the upstream built-in default enablement; --enable=all maxima never enabled"
GOFUMPT_POLICY = "strict superset of gofmt; whole-file rewrite with check/diff mode, no rule-set selection"
CLANG_FORMAT_POLICY = "authoritative-toolchain formatter; deterministic format, no rule-set selection"
BEYOND_DEFAULT_REJECTED = "beyond-default switches rejected: staticcheck SA-only preset plus -all, govet all analyzers, clang-tidy --checks=*, cppcheck --enable=all"

NATIVE_FIXTURE_CC_HELLO = "//cc/tests/fixtures/hello:hello_test"
NATIVE_FIXTURE_GO_HELLO = "//go/tests/fixtures/hello:hello_test"

NATIVE_REJECTED = "hidden presets rejected: no SA-only preset, no preset checks/enablement/analyzers; unpinned versions rejected: no floating version or head; --enable=all-style maxima rejected"
