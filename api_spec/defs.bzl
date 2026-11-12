"""Public API spec API."""

load(
    "//api_spec/private:defs.bzl",
    _ApiSpecInfo = "ApiSpecInfo",
    _api_spec_library = "api_spec_library",
)
load(
    "//api_spec/private:codegen.bzl",
    _api_spec_typescript_client = "api_spec_typescript_client",
    _api_spec_go_client = "api_spec_go_client",
    _api_spec_python_client = "api_spec_python_client",
    _api_spec_rust_client = "api_spec_rust_client",
    _api_spec_codegen_shard = "api_spec_codegen_shard",
)

ApiSpecInfo = _ApiSpecInfo
api_spec_library = _api_spec_library
api_spec_codegen_shard = _api_spec_codegen_shard
api_spec_go_client = _api_spec_go_client
api_spec_python_client = _api_spec_python_client
api_spec_rust_client = _api_spec_rust_client
api_spec_typescript_client = _api_spec_typescript_client
