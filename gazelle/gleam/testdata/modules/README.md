# Gleam Package Generation

One `gleam_library` for `src/` and one `gleam_test` for `test/*_test.gleam` in
each directory with a `gleam.toml`. Imports resolve to the library that owns
the module. `gleam/*` modules resolve through `gleam_stdlib`. Other `test/`
modules go into a testonly `gleam_test_library` named `<name>_test_support`,
which the test depends on.
