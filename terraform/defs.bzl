"""Public terraform API."""

load(
    "//terraform/private:defs.bzl",
    _collect_providers = "collect_providers",
    _terraform_library = "terraform_library",
)
load("//terraform/private:providers.bzl", _terraform_provider = "terraform_provider")

collect_providers = _collect_providers
terraform_library = _terraform_library
terraform_provider = _terraform_provider
