"""Public FlatBuffers API."""

load(
    "//flatbuffers/private:defs.bzl",
    _FlatBuffersInfo = "FlatBuffersInfo",
    _flatbuffers_library = "flatbuffers_library",
)
load(
    "//flatbuffers/private:codegen.bzl",
    _flatbuffers_cc_library = "flatbuffers_cc_library",
    _flatbuffers_java_library = "flatbuffers_java_library",
    _flatbuffers_rust_library = "flatbuffers_rust_library",
    _flatbuffers_codegen_shard = "flatbuffers_codegen_shard",
)
load(
    "//flatbuffers/private:conform.bzl",
    _flatbuffers_conform_test = "flatbuffers_conform_test",
)

FlatBuffersInfo = _FlatBuffersInfo
flatbuffers_cc_library = _flatbuffers_cc_library
flatbuffers_codegen_shard = _flatbuffers_codegen_shard
flatbuffers_conform_test = _flatbuffers_conform_test
flatbuffers_java_library = _flatbuffers_java_library
flatbuffers_library = _flatbuffers_library
flatbuffers_rust_library = _flatbuffers_rust_library
