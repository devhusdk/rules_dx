"""Providers for Rego rules."""

RegoInfo = provider(
    doc = "Transitive Rego sources and data of a rego_library.",
    fields = {
        "transitive_data": "depset[File]: data files (.json, .yaml, .yml) of this library and its deps.",
        "transitive_srcs": "depset[File]: .rego files of this library and its deps.",
    },
)
