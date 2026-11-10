"""Public Cap'n Proto API."""

load(
    "//capnproto/private:providers.bzl",
    _CapnpInfo = "CapnpInfo",
)
load(
    "//capnproto/private:defs.bzl",
    _capnp_library = "capnp_library",
    _std_import_path = "std_import_path",
)
load(
    "//capnproto/private:codegen.bzl",
    _capnp_cc_library = "capnp_cc_library",
    _capnp_java_library = "capnp_java_library",
    _capnp_rust_library = "capnp_rust_library",
    _capnp_codegen_shard = "capnp_codegen_shard",
)

CapnpInfo = _CapnpInfo
capnp_cc_library = _capnp_cc_library
capnp_codegen_shard = _capnp_codegen_shard
capnp_java_library = _capnp_java_library
capnp_library = _capnp_library
capnp_rust_library = _capnp_rust_library
std_import_path = _std_import_path
