"""Public Cap'n Proto API."""

load(
    "//capnproto/private:providers.bzl",
    _CapnpInfo = "CapnprotoInfo",
)
load(
    "//capnproto/private:defs.bzl",
    _capnproto_library = "capnproto_library",
)
load(
    "//capnproto/private:codegen.bzl",
    _capnproto_cc_library = "capnproto_cc_library",
    _capnproto_java_library = "capnproto_java_library",
    _capnproto_rust_library = "capnproto_rust_library",
    _capnproto_codegen_shard = "capnproto_codegen_shard",
)

CapnprotoInfo = _CapnpInfo
capnproto_cc_library = _capnproto_cc_library
capnproto_codegen_shard = _capnproto_codegen_shard
capnproto_java_library = _capnproto_java_library
capnproto_library = _capnproto_library
capnproto_rust_library = _capnproto_rust_library
