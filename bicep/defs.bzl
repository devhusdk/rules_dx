"""Public bicep API."""

load(
    "//bicep/private:defs.bzl",
    _bicep_library = "bicep_library",
    _bicep_registry_module = "bicep_registry_module",
)

bicep_library = _bicep_library
bicep_registry_module = _bicep_registry_module
