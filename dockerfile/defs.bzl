"""Public dockerfile API."""

load(
    "//dockerfile/private:defs.bzl",
    _dockerfile_library = "dockerfile_library",
)

dockerfile_library = _dockerfile_library
