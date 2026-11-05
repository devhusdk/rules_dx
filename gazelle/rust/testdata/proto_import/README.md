# Rust Proto Import

Verifies that a `.proto` file gets a `rust_prost_library` next to its `proto_library` when
`rules_rust_prost` is a module dependency, and that a crate's `use api_proto_rs::` resolves to it.
