"""Public cloudformation API."""

load(
    "//cloudformation/private:defs.bzl",
    _cloudformation_library = "cloudformation_library",
)

cloudformation_library = _cloudformation_library
