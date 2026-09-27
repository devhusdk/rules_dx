"""Rust foundation pins plus crate-manifest groups."""

load(":versions.bzl", _RULES_RUST_PROST_VERSION = "RULES_RUST_PROST_VERSION", _RULES_RUST_VERSION = "RULES_RUST_VERSION", _RUSTFMT_VERSION = "RUSTFMT_VERSION", _RUST_VERSION = "RUST_VERSION")

RULES_RUST_VERSION = _RULES_RUST_VERSION
RULES_RUST_PROST_VERSION = _RULES_RUST_PROST_VERSION
RUST_VERSION = _RUST_VERSION
RUSTFMT_VERSION = _RUSTFMT_VERSION

CRATE_FIXTURE_MANIFESTS = [
    "//rust/tests/fixtures/hello:Cargo.toml",
]

CRATE_CLI_MANIFESTS = [
    "//cli/process:Cargo.toml",
    "//cli/apply:Cargo.toml",
    "//cli/atomic_fs:Cargo.toml",
    "//cli/adopt:Cargo.toml",
    "//cli/setup:Cargo.toml",
    "//cli/clean:Cargo.toml",
    "//cli/audit:Cargo.toml",
    "//cli/output:Cargo.toml",
    "//cli/path:Cargo.toml",
    "//cli/proto_validate:Cargo.toml",
    "//cli/schema:Cargo.toml",
    "//cli/bep:Cargo.toml",
    "//cli/cli:Cargo.toml",
    "//cli/diff:Cargo.toml",
    "//cli/digest:Cargo.toml",
    "//cli/lcov:Cargo.toml",
    "//cli/env:Cargo.toml",
    "//cli/codegen:Cargo.toml",
    "//cli/env_plan:Cargo.toml",
    "//cli/fingerprint:Cargo.toml",
    "//cli/test_scratch:Cargo.toml",
    "//cli/roots:Cargo.toml",
    "//cli/docgen:Cargo.toml",
    "//cli/update:Cargo.toml",
    "//cli/bump:Cargo.toml",
    "//cli/ci:Cargo.toml",
    "//cli/qualification:Cargo.toml",
]

CRATE_DEPLOY_MANIFESTS = [
    "//deploy/rules:Cargo.toml",
    "//deploy/release:Cargo.toml",
    "//deploy/install:Cargo.toml",
]

CRATE_SHARD_WRITER_MANIFESTS = [
    "//env/env_shard:Cargo.toml",
    "//generation/codegen_shard:Cargo.toml",
]

CRATE_SHARED_MANIFESTS = [
    "//generation/result:Cargo.toml",
    "//quality/adapter:Cargo.toml",
    "//quality/evaluator:Cargo.toml",
    "//quality/markdown:Cargo.toml",
    "//quality/result:Cargo.toml",
    "//quality/runner:Cargo.toml",
    "//docs/ir/ir:Cargo.toml",
    "//docs/adapters:Cargo.toml",
    "//tools/bazelrc:Cargo.toml",
    "//tools/depcheck:Cargo.toml",
]

RUST_CRATE_MANIFESTS = (
    CRATE_FIXTURE_MANIFESTS +
    CRATE_CLI_MANIFESTS +
    CRATE_DEPLOY_MANIFESTS +
    CRATE_SHARD_WRITER_MANIFESTS +
    CRATE_SHARED_MANIFESTS
)

CRATE_REPIN = "CARGO_BAZEL_REPIN=1 bazel build //rust/tests/fixtures/hello:hello"
CRATE_CARGO_LOCK = "//rust/tests/fixtures/hello:Cargo.lock"
CRATE_BAZEL_LOCK = "//:cargo-bazel-lock.json"

SHELL_ENV_GENERATOR_DEFAULT = 0
SHELL_ENV_GLOBAL_FLAG = False
