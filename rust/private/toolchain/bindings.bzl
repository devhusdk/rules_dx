"""Authoritative Rust toolchain bindings for quality adapters."""

RUST_TOOLCHAIN_TYPE = "@rules_rust//rust:toolchain_type"
RUSTFMT_TOOLCHAIN_TYPE = "@rules_rust//rust/rustfmt:toolchain_type"

def rust_toolchain_tools(ctx):
    """Return the (clippy_driver, rustfmt) Files for the selected toolchain."""
    rust = ctx.toolchains[RUST_TOOLCHAIN_TYPE]
    rustfmt = ctx.toolchains[RUSTFMT_TOOLCHAIN_TYPE]
    return rust.clippy_driver, rustfmt.rustfmt

def rust_toolchain_rustc(ctx):
    """Return the rustc File for the selected toolchain."""
    return ctx.toolchains[RUST_TOOLCHAIN_TYPE].rustc

def rust_toolchain_toolchains():
    """Toolchain types an adapter rule must declare for rust_toolchain_tools."""
    return [RUST_TOOLCHAIN_TYPE, RUSTFMT_TOOLCHAIN_TYPE]

def rust_toolchain_runtime_files(ctx):
    """Return the shared libraries the selected toolchains' rustfmt and clippy_driver load."""
    rust = ctx.toolchains[RUST_TOOLCHAIN_TYPE]
    rustfmt = ctx.toolchains[RUSTFMT_TOOLCHAIN_TYPE]
    return depset(transitive = [rust.rustc_lib, getattr(rustfmt, "rustc_lib", depset())]).to_list()
