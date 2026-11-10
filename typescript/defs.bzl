"""Public TypeScript API."""

load(
    "//typescript/private:defs.bzl",
    _typescript_srcs_rejection = "typescript_srcs_rejection",
    _typescript_declaration_srcs_rejection = "typescript_declaration_srcs_rejection",
    _typescript_upstream_srcs = "typescript_upstream_srcs",
    _typescript_empty_library_rejection = "typescript_empty_library_rejection",
    _typescript_transpiler = "typescript_transpiler",
    _dx_tsconfig_is_source = "dx_tsconfig_is_source",
    _dx_tsconfig_relpath = "dx_tsconfig_relpath",
    _dx_tsconfig_content = "dx_tsconfig_content",
    _typescript_scoped_tsconfig_rejection = "typescript_scoped_tsconfig_rejection",
    _dx_scoped_tsconfig_out = "dx_scoped_tsconfig_out",
    _typescript_library = "typescript_library",
    _typescript_proto_library = "typescript_proto_library",
    _typescript_binary_main = "typescript_binary_main",
    _typescript_binary_rejection = "typescript_binary_rejection",
    _typescript_binary_kwargs_rejection = "typescript_binary_kwargs_rejection",
    _typescript_compiled_entry = "typescript_compiled_entry",
    _typescript_binary_upstream_kwargs = "typescript_binary_upstream_kwargs",
    _typescript_binary_forward_kwargs = "typescript_binary_forward_kwargs",
    _typescript_binary = "typescript_binary",
    _typescript_test_rejection = "typescript_test_rejection",
    _typescript_test = "typescript_test",
    _typescript_bench = "typescript_bench",
    _typescript_fuzz_test = "typescript_fuzz_test",
    _typescript_playwright_test = "typescript_playwright_test",
)

typescript_srcs_rejection = _typescript_srcs_rejection
typescript_declaration_srcs_rejection = _typescript_declaration_srcs_rejection
typescript_upstream_srcs = _typescript_upstream_srcs
typescript_empty_library_rejection = _typescript_empty_library_rejection
typescript_transpiler = _typescript_transpiler
dx_tsconfig_is_source = _dx_tsconfig_is_source
dx_tsconfig_relpath = _dx_tsconfig_relpath
dx_tsconfig_content = _dx_tsconfig_content
typescript_scoped_tsconfig_rejection = _typescript_scoped_tsconfig_rejection
dx_scoped_tsconfig_out = _dx_scoped_tsconfig_out
typescript_library = _typescript_library
typescript_proto_library = _typescript_proto_library
typescript_binary_main = _typescript_binary_main
typescript_binary_rejection = _typescript_binary_rejection
typescript_binary_kwargs_rejection = _typescript_binary_kwargs_rejection
typescript_compiled_entry = _typescript_compiled_entry
typescript_binary_upstream_kwargs = _typescript_binary_upstream_kwargs
typescript_binary_forward_kwargs = _typescript_binary_forward_kwargs
typescript_binary = _typescript_binary
typescript_test_rejection = _typescript_test_rejection
typescript_test = _typescript_test
typescript_bench = _typescript_bench
typescript_fuzz_test = _typescript_fuzz_test
typescript_playwright_test = _typescript_playwright_test
