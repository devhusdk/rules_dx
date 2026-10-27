"""Pinned nightly rustdoc toolchains for the Rust API reference."""

load("@rules_rust//rust/private:repositories.bzl", "rust_repository_set")

RUSTDOC_NIGHTLY = "nightly/2026-08-20"

_HUB_NAME = "dx_rustdoc_toolchains"

_EXEC_TRIPLES = [
    "aarch64-apple-darwin",
    "aarch64-pc-windows-msvc",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
]

_CHANNEL = "@rules_rust//rust/toolchain/channel:nightly"

_TOOLCHAIN_TYPE = "@rules_rust//rust:toolchain"

def hub_build(details):
    """Returns the hub BUILD file declaring one nightly toolchain per exec triple."""
    lines = []
    for name in sorted(details.keys()):
        detail = details[name]
        lines.append("toolchain(")
        lines.append("    name = %r," % name)
        lines.append("    exec_compatible_with = %r," % detail["exec_compatible_with"])
        lines.append("    target_compatible_with = %r," % detail["target_compatible_with"])
        lines.append("    target_settings = [%r]," % _CHANNEL)
        lines.append("    toolchain = %r," % detail["tools_toolchain_label"])
        lines.append("    toolchain_type = %r," % _TOOLCHAIN_TYPE)
        lines.append('    visibility = ["//visibility:public"],')
        lines.append(")")
        lines.append("")
    return "\n".join(lines)

def _hub_repo_impl(ctx):
    ctx.file("BUILD.bazel", ctx.attr.build)
    return ctx.repo_metadata(reproducible = True)

_hub_repo = repository_rule(
    implementation = _hub_repo_impl,
    attrs = {"build": attr.string(
        doc = "BUILD file content written to the hub repository root.",
        mandatory = True,
    )},
)

def _dx_rustdoc_impl(module_ctx):
    details = {}
    for triple in _EXEC_TRIPLES:
        details.update(rust_repository_set(
            name = "dx_rustdoc_" + triple.replace("-", "_"),
            edition = "2021",
            exec_triple = triple,
            versions = [RUSTDOC_NIGHTLY],
        ))
    _hub_repo(name = _HUB_NAME, build = hub_build(details))
    return module_ctx.extension_metadata(reproducible = True)

dx_rustdoc = module_extension(
    implementation = _dx_rustdoc_impl,
)
