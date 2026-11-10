"""Analysis subject proving rustfmt/Clippy/rustc bind to the toolchain."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load(":bindings.bzl", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")

def _toolchain_identity_subject_impl(ctx):
    clippy_driver, rustfmt = rust_toolchain_tools(ctx)
    rustc = rust_toolchain_rustc(ctx)
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "clippy.owner": str(clippy_driver.owner),
            "clippy.path": clippy_driver.path,
            "rustc.owner": str(rustc.owner),
            "rustc.path": rustc.path,
            "rustfmt.owner": str(rustfmt.owner),
            "rustfmt.path": rustfmt.path,
        }),
    ]

toolchain_identity_subject = rule(
    implementation = _toolchain_identity_subject_impl,
    doc = "Analysis subject reporting the Rust toolchain tools.",
    toolchains = rust_toolchain_toolchains(),
)
