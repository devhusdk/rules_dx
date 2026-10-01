"""yamlfmt check plus fix wiring."""

YAMLFMT_VERSION = "v0.21.0"
YAMLFMT_ARTIFACT = "standalone checksummed release artifact; -lint check"
YAMLFMT_CHECK = "yamlfmt -lint -q"
YAMLFMT_FIX = "yamlfmt"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/yamlfmt:corpus_starlark"
YAMLFMT_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
