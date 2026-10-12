"""Selective Go dx update per-module pins."""

SELECTIVE_GO = "wont-fix"
GO_FULL = "noop"

GO_FULL_LOCK = "third_party/go/go.mod plus go.sum via go_deps.from_file tracks Gazelle; no launch"
GO_FROM_FILE = "go_deps.from_file"
GO_MOD = "third_party/go/go.mod"
GO_SUM = "third_party/go/go.sum"

SELECTIVE_GO_SELECTOR = "go:github.com/google/go-cmp/cmp"
SELECTIVE_GO_SELECTOR_LABEL = "go:<module-path>"

SELECTIVE_GO_HINT = "widen explicitly via `dx bump go:<module> <version>`"

BUMP_FOLLOWUP_GO = "dx update go"

REJECTED_SILENT_FULL_SUBSTITUTION = "silent full-update substitution rejected"
REJECTED_PRIVATE_GO_GET = "private go get rejected"
REJECTED_PRIVATE_GO_MOD_TIDY = "private go mod tidy rejected"

NO_SUPPORTED = "no Supported claim"
SEED_ONLY = "qualified seed-only"
