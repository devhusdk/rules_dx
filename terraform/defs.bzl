"""Public terraform API."""

load(
    "//terraform/private:defs.bzl",
    _terraform_library = "terraform_library",
)
load("//terraform/private:providers.bzl", _terraform_provider = "terraform_provider")

terraform_library = _terraform_library
terraform_provider = _terraform_provider
