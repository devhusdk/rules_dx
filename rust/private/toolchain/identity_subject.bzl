"""Analysis subject proving rustfmt/Clippy/rustc bind to the toolchain."""

load("//core:defs.bzl", "DxSubjectInfo")
load(":bindings.bzl", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")

def _configuration_free(ctx, path):
    prefix = ctx.bin_dir.path + "/"
    return path[len(prefix):] if path.startswith(prefix) else path

def _toolchain_identity_subject_impl(ctx):
    clippy_driver, rustfmt = rust_toolchain_tools(ctx)
    rustc = rust_toolchain_rustc(ctx)
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "clippy.owner": str(clippy_driver.owner),
            "clippy.path": _configuration_free(ctx, clippy_driver.path),
            "rustc.owner": str(rustc.owner),
            "rustc.path": _configuration_free(ctx, rustc.path),
            "rustfmt.owner": str(rustfmt.owner),
            "rustfmt.path": _configuration_free(ctx, rustfmt.path),
        }),
    ]

toolchain_identity_subject = rule(
    implementation = _toolchain_identity_subject_impl,
    doc = "Analysis subject reporting the Rust toolchain tools.",
    toolchains = rust_toolchain_toolchains(),
)
