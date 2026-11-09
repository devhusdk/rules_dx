# Rust

Rust test fixtures. They live in the `rules_dx_rust` module, next to
`rules_dx`. The rules, the toolchains, the lint, format and typecheck aspects,
the Gazelle language and the wasm and WASI support stay in `rules_dx`, because
its own tools are built with them. Fixtures that only exercise consumer Rust
code live here. `rust/tests/fixtures/wasm_bindgen` stays in `rules_dx` because
it tests the core rust and javascript wasm-bindgen rules.
`rust/tests/fixtures/shared-rust-checks` stays in `rules_dx` because it spans
the android, rust, javascript and wasi families, and family modules may not
depend on each other.
