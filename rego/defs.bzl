"""Public rego API."""

load(
    "//rego/private:defs.bzl",
    _rego_library = "rego_library",
    _rego_test = "rego_test",
)

rego_library = _rego_library
rego_test = _rego_test
