"""terraform check plus fix wiring."""

TERRAFORM_VERSION = "v1.16.1"
TERRAFORM_ARTIFACT = "standalone checksummed release artifact; terraform fmt"
TERRAFORM_CHECK = "terraform fmt -check"
TERRAFORM_FIX = "terraform fmt"
FILE_FAMILY_PROOF = "bazel build //quality/tests/fixtures/file_family_adapters/terraform:corpus_starlark"
TERRAFORM_REJECTED = "ambient discovery rejected; auto-supplied preset rejected"
