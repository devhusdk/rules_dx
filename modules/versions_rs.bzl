"""Writes the versions dx init scaffolds as a Rust source file."""

load(
    ":versions.bzl",
    "ASPECT_RULES_JS_VERSION",
    "ASPECT_RULES_TS_VERSION",
    "CUDA_VERSION",
    "LLVM_VERSION",
    "RULES_CC_VERSION",
    "RULES_CUDA_VERSION",
    "RULES_PY_VERSION",
    "RULES_RUST_VERSION",
    "RUSTFMT_VERSION",
    "RUST_EDITION",
    "RUST_VERSION",
    "TYPESCRIPT_INTEGRITY",
    "TYPESCRIPT_VERSION",
)

_CONSTS = [
    ("ASPECT_RULES_JS_VERSION", ASPECT_RULES_JS_VERSION),
    ("ASPECT_RULES_TS_VERSION", ASPECT_RULES_TS_VERSION),
    ("CUDA_VERSION", CUDA_VERSION),
    ("LLVM_VERSION", LLVM_VERSION),
    ("RULES_CC_VERSION", RULES_CC_VERSION),
    ("RULES_CUDA_VERSION", RULES_CUDA_VERSION),
    ("RULES_PY_VERSION", RULES_PY_VERSION),
    ("RULES_RUST_VERSION", RULES_RUST_VERSION),
    ("RUSTFMT_VERSION", RUSTFMT_VERSION),
    ("RUST_EDITION", RUST_EDITION),
    ("RUST_VERSION", RUST_VERSION),
    ("TYPESCRIPT_INTEGRITY", TYPESCRIPT_INTEGRITY),
    ("TYPESCRIPT_VERSION", TYPESCRIPT_VERSION),
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
