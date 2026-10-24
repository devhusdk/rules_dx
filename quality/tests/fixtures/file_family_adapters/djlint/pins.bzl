"""djlint check plus fix wiring."""

DJLINT_VERSION = "v1.45.2"
DJLINT_ARTIFACT = "private wheel-only Python graph member"
DJLINT_CHECK = "djlint --lint plus --reformat --check"
DJLINT_FIX = "djlint --reformat"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/djlint:corpus_starlark"
DJLINT_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
