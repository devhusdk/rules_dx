"""Providers for Bicep rules."""

BicepInfo = provider(
    doc = "Transitive Bicep sources and data of a bicep_library.",
    fields = {
        "transitive_data": "depset[File]: data files read by load functions of this module and its deps.",
        "transitive_registry": "depset[File]: cached registry module files of this module and its deps.",
        "transitive_srcs": "depset[File]: .bicep and .bicepparam files of this module and its deps.",
    },
)

BicepRegistryInfo = provider(
    doc = "A restored Bicep registry module.",
    fields = {
        "cache_root": "string: repository-relative root of the module cache.",
        "files": "depset[File]: cached module files.",
        "ref": "string: br: module reference.",
    },
)
