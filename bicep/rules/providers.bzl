"""Providers for Bicep rules."""

BicepInfo = provider(
    doc = "Transitive Bicep sources and data of a bicep_module.",
    fields = {
        "transitive_data": "depset[File]: data files read by load functions of this module and its deps.",
        "transitive_srcs": "depset[File]: .bicep and .bicepparam files of this module and its deps.",
    },
)
