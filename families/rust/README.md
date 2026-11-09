# Rust

Rust test fixtures. They live in the `rules_dx_rust` module, next to
`rules_dx`. The rules, the toolchains, the lint, format and typecheck aspects,
the Gazelle language and the wasm and WASI support stay in `rules_dx`, because
its own tools are built with them. The fixtures that read the shared
`dx_crates` hub stay there too.
