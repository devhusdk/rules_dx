"""modfmt check plus fix wiring."""

MODFMT_VERSION = "v0.4.0"
MODFMT_ARTIFACT = "standalone checksummed release artifact from github.com/joshdk/modfmt v0.4.0"
MODFMT_CHECK = "modfmt -c -l"
MODFMT_FIX = "modfmt -w"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/modfmt:corpus_starlark"
MODFMT_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
