"""Go from_file lock wiring pins."""

load("//modules:versions.bzl", _GAZELLE_VERSION = "GAZELLE_VERSION", _GO_LANGUAGE_FLOOR = "GO_LANGUAGE_FLOOR", _GO_SDK_VERSION = "GO_SDK_VERSION", _RULES_GO_VERSION = "RULES_GO_VERSION")

RULES_GO_VERSION = _RULES_GO_VERSION

GO_SDK_VERSION = _GO_SDK_VERSION

GO_LANGUAGE_FLOOR = _GO_LANGUAGE_FLOOR

GAZELLE_VERSION = _GAZELLE_VERSION

GODEPS_FROM_FILE = "go_deps.from_file"
GODEPS_GO_MOD = "//third_party/go:go.mod"
GODEPS_GO_SUM = "//third_party/go:go.sum"
GODEPS_GO_WORK_ABSENT = "go.work only for multi-module"

BUILDTOOLS_VERSION = "v0.0.0-20250930140053-2eb4fccefb52"
GO_CMP_VERSION = "v0.6.0"
DIFLIB_VERSION = "v1.0.0"

GODEPS_REPO_BUILDTOOLS = "@com_github_bazelbuild_buildtools"
GODEPS_REPO_GO_CMP = "@com_github_google_go_cmp"
GODEPS_REPO_DIFLIB = "@com_github_pmezard_go_difflib"

GODEPS_FIXTURE_LIB = "//go/tests/fixtures/godeps:godeps"
GODEPS_FIXTURE_TEST = "//go/tests/fixtures/godeps:godeps_test"

GODEPS_IMPORT = "github.com/google/go-cmp/cmp"
GODEPS_LABEL = "@com_github_google_go_cmp//cmp:cmp"

GODEPS_GENERATION = "consumes never writes"

GODEPS_REJECTED = "hand module tags rejected: from_file preferred; go.work only for multi-module"

GODEPS_CURRENCY_RECHECK = "2026-09-22"
