# Rust

Rust test fixtures. They live in the `rules_dx_rust` module, next to
`rules_dx`. The rules, the toolchains, the lint, format and typecheck aspects,
the Gazelle language and the wasm and WASI support stay in `rules_dx`.
Fixtures that only exercise consumer Rust code live here.
`rust/tests/fixtures/wasm_bindgen` and `rust/tests/fixtures/shared-rust-checks`
stay in `rules_dx`.
