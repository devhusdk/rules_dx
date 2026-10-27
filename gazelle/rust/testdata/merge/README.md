# Rust Merge And Cleanup

Verifies generated attribute refresh, user-owned attribute preservation, and conservative stale-rule cleanup.

Matrix: `merge` (source rename with rule/attribute/value `# keep`, unfamiliar attrs, stale lib+test removal), `binary` (binary refresh, unit-test wrapper merge, stale binary+test removal), `cargo_trim` (manifest-entry removal drops the stale test rule), `empty` (comment-only BUILD passes through unchanged), `legacy_crate_deps` (old `crate_deps` call moves to `all_crate_deps` and keeps hand labels), `unit_shape` (an outer `#[cfg(test)] mod tests;` file leaves the library srcs and the `crate=` unit test becomes self-contained, keeping its name and inheriting `compile_data`).
