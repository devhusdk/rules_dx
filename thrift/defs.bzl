"""Public Thrift API."""

load(
    "//thrift/private:defs.bzl",
    _ThriftInfo = "ThriftInfo",
    _thrift_library = "thrift_library",
)
load(
    "//thrift/private:codegen.bzl",
    _thrift_cc_library = "thrift_cc_library",
    _thrift_java_library = "thrift_java_library",
    _thrift_rust_library = "thrift_rust_library",
    _thrift_codegen_shard = "thrift_codegen_shard",
)

ThriftInfo = _ThriftInfo
thrift_cc_library = _thrift_cc_library
thrift_codegen_shard = _thrift_codegen_shard
thrift_java_library = _thrift_java_library
thrift_library = _thrift_library
thrift_rust_library = _thrift_rust_library
