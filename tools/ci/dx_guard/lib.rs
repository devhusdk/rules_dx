//! What the source and test guards in `tools/ci` share: the snapshot, the Starlark walk, the Rust lexer, the detection of test calls and the allowlist check.

pub mod allowlist;
pub mod lexer;
pub mod snapshot;
pub mod starlark;
pub mod test_calls;
