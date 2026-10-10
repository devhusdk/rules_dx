"""Staticcheck check-only wiring."""

STATICCHECK_VERSION = "2026.2"

STATICCHECK_ARTIFACT = "standalone checksummed release artifact"

STATICCHECK_CHECK = "staticcheck -f json with one JSON object per stdout line (exit 0 clean with empty stdout, exit 1 with finding objects when dirty)"
STATICCHECK_FIX = "check-only with the provisional sandbox-apply-and-diff fix flow"
STATICCHECK_CONFIG_POLICY = "default checks are the upstream built-in default checks, not an SA-only preset; SA-only shortcut rejected without qualification"

GO_FIXTURE_HELLO = "//go/tests/fixtures/hello:hello_test"

STATICCHECK_REJECTED = "SA-only shortcut rejected without qualification; -all maxima rejected; SARIF ingestion rejected (native JSON is the faithful shape)"
