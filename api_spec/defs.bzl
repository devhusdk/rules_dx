"""Public API spec API."""

load(
    "//api_spec/private:defs.bzl",
    _ApiSpecInfo = "ApiSpecInfo",
    _api_spec_library = "api_spec_library",
)
load(
    "//api_spec/private:codegen.bzl",
    _openapi_typescript_client = "openapi_typescript_client",
    _openapi_go_client = "openapi_go_client",
    _openapi_python_client = "openapi_python_client",
    _openapi_rust_client = "openapi_rust_client",
    _openapi_codegen_shard = "openapi_codegen_shard",
)

ApiSpecInfo = _ApiSpecInfo
api_spec_library = _api_spec_library
openapi_codegen_shard = _openapi_codegen_shard
openapi_go_client = _openapi_go_client
openapi_python_client = _openapi_python_client
openapi_rust_client = _openapi_rust_client
openapi_typescript_client = _openapi_typescript_client
