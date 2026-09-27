"""Go test runner pins."""

load("//modules:versions.bzl", _GO_SDK_VERSION = "GO_SDK_VERSION", _RULES_GO_VERSION = "RULES_GO_VERSION")

RULES_GO_VERSION = _RULES_GO_VERSION

GO_SDK_VERSION = _GO_SDK_VERSION

GOTEST_KIND = "go_test"
GOTEST_RUNNER = "go test"
GOTEST_EMBED_ATTR = "embed"

GOTEST_FIXTURE_LABEL = "//go/tests/fixtures/hello:hello_test"

GOTEST_REJECTED = "implicit runner rejected: no bare go test without go_test plus embed"
