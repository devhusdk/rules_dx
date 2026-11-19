//! What the source and test guards in `tools/ci` share: the snapshot, the Starlark walk, the Rust lexer, the masked Starlark call scan, the split into code, comments and strings, the detection of test calls and the allowlist check.

pub mod allowlist;
pub mod lexer;
pub mod masked;
pub mod segments;
pub mod snapshot;
pub mod starlark;
pub mod test_calls;
