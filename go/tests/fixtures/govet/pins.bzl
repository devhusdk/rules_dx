"""Govet check-only wiring with errcheck complementary."""

load("//modules:versions.bzl", _GO_SDK_VERSION = "GO_SDK_VERSION", _RULES_GO_VERSION = "RULES_GO_VERSION")

GOVET_TOOLCHAIN_VERSION = _GO_SDK_VERSION

GOVET_COUPLING = "ships with the qualified Go toolchain (rules_go " + _RULES_GO_VERSION + " plus Go SDK " + _GO_SDK_VERSION + "), no separate acquisition"

GOVET_CHECK = "go vet text diagnostics file:line:col: message on stderr (exit 0 clean, exit 1 with findings)"
GOVET_FIX = "check-only with the provisional sandbox-apply-and-diff fix flow"
GOVET_CONFIG_POLICY = "default analyzers are the upstream built-in default analyzers; all-analyzer and vettool maxima never enabled"

GO_FIXTURE_HELLO = "//go/tests/fixtures/hello:hello_test"

GOVET_REJECTED = "all-analyzer maxima rejected; exit-code-only classification rejected"
