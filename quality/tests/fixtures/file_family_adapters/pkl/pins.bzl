"""pkl check plus fix wiring."""

PKL_VERSION = "0.32.1"
PKL_ARTIFACT = "standalone checksummed release artifact"
PKL_CHECK = "pkl format --diff-name-only"
PKL_FIX = "pkl format -w"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/pkl:corpus_starlark"
PKL_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
