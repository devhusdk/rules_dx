# Cargo-Aware Rust Generation

Verifies authoritative Cargo names, paths, editions, dependency scopes, and custom test harnesses.

`shapes` covers examples with and without `test = true` plus a `harness = false` bench;
`proc_macro`, `cdylib`, and `staticlib` cover library flavors; `scripted` covers an explicit
build script and its consumer edge.

`suite` covers implicit `tests/*.rs` roots, which must be emitted in sorted order.
