"""Writes the versions dx init scaffolds as a Rust source file."""

load(
    ":versions.bzl",
    "FLOOR_LLVM",
    "FLOOR_RULES_CC",
    "FLOOR_RULES_RUST",
    "RUSTFMT_VERSION",
    "RUST_EDITION",
    "RUST_VERSION",
)

_CONSTS = [
    ("FLOOR_LLVM", FLOOR_LLVM),
    ("FLOOR_RULES_CC", FLOOR_RULES_CC),
    ("FLOOR_RULES_RUST", FLOOR_RULES_RUST),
    ("RUSTFMT_VERSION", RUSTFMT_VERSION),
    ("RUST_EDITION", RUST_EDITION),
    ("RUST_VERSION", RUST_VERSION),
]

def _versions_rs_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".rs")
    lines = ["//! Versions from `modules/versions.bzl`.", ""]
    for name, value in _CONSTS:
        lines.append("pub const {}: &str = {};".format(name, json.encode(value)))
    ctx.actions.write(out, "\n".join(lines) + "\n")
    return [DefaultInfo(files = depset([out]))]

versions_rs = rule(
    implementation = _versions_rs_impl,
    doc = "Writes version pins as Rust constants.",
)
