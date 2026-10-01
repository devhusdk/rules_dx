"""standardrb check plus fix wiring."""

STANDARDRB_VERSION = "1.56.0"
STANDARDRB_ARTIFACT = "release-assembled Ruby closure member"
STANDARDRB_CHECK = "standardrb --format json"
STANDARDRB_FIX = "standardrb --fix"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/standardrb:corpus_starlark"
STANDARDRB_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
