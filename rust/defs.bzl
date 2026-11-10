"""Public Rust API."""

load(
    "//rust/private:defs.bzl",
    _rust_library = "rust_library",
    _rust_binary = "rust_binary",
    _rust_test = "rust_test",
    _rust_bench = "rust_bench",
    _rust_proc_macro = "rust_proc_macro",
    _rust_shared_library = "rust_shared_library",
    _rust_static_library = "rust_static_library",
    _rust_fuzz_test = "rust_fuzz_test",
)

rust_library = _rust_library
rust_binary = _rust_binary
rust_test = _rust_test
rust_bench = _rust_bench
rust_proc_macro = _rust_proc_macro
rust_shared_library = _rust_shared_library
rust_static_library = _rust_static_library
rust_fuzz_test = _rust_fuzz_test
