"""keep_sorted check plus fix wiring."""

KEEP_SORTED_VERSION = "v0.10.0"
KEEP_SORTED_ARTIFACT = "standalone checksummed release artifact; check-only"
KEEP_SORTED_CHECK = "keep-sorted --mode lint JSON (exit 1 dirty with findings, exit 0 clean with empty output)"
KEEP_SORTED_FIX = "keep-sorted --mode fix in-place rewrite (re-read on exit 0, keep input otherwise)"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/keep_sorted:corpus_starlark"
KEEP_SORTED_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
