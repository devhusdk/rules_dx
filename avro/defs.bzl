"""Public Avro API."""

load(
    "//avro/private:providers.bzl",
    _AvroInfo = "AvroInfo",
)
load(
    "//avro/private:defs.bzl",
    _avro_library = "avro_library",
)
load(
    "//avro/private:codegen.bzl",
    _avro_cc_library = "avro_cc_library",
    _avro_java_library = "avro_java_library",
    _avro_codegen_shard = "avro_codegen_shard",
)

AvroInfo = _AvroInfo
avro_cc_library = _avro_cc_library
avro_codegen_shard = _avro_codegen_shard
avro_java_library = _avro_java_library
avro_library = _avro_library
