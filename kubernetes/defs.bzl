"""Public kubernetes API."""

load(
    "//kubernetes/private:defs.bzl",
    _kubernetes_library = "kubernetes_library",
    _kubernetes_schemas = "kubernetes_schemas",
)

kubernetes_library = _kubernetes_library
kubernetes_schemas = _kubernetes_schemas
