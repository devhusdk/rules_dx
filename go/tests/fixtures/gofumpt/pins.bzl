"""Gofumpt check plus fix wiring."""

GOFUMPT_VERSION = "v0.12.0"

GOFUMPT_ARTIFACT = "built from mvdan.cc/gofumpt through go_deps with the hermetic rules_go toolchain; strict superset of gofmt"

GOFUMPT_CHECK = "gofumpt -lang=<sdk go1.N> -d with unified diff on stdout (exit 0 clean with empty diff, exit 1 with diff headers when dirty, exit 2 on a syntax error)"
GOFUMPT_FIX = "gofumpt -lang=<sdk go1.N> -w in-place rewrite (re-read on exit 0, keep input otherwise)"
GOFUMPT_CONFIG_POLICY = "strict superset of gofmt; whole-file rewrite with check/diff mode, no rule-set selection, no config file"

GO_FIXTURE_HELLO = "//go/tests/fixtures/hello:hello_test"

GOFUMPT_REJECTED = "gofmt-only scope rejected; exit-code-only classification rejected; auto-supplied preset rejected"
