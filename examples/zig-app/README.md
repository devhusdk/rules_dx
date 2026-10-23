# Zig manifest example

A multi-package Zig app. It has a `manifest` CLI, a catalog module with a generated `schema` module and an embedded `items.tsv`, and a digest module that calls C through a pinned `xxHash` dependency.

| Package | Role |
| --- | --- |
| `catalog` | TSV parser and totals, generated `schema.zig`, embedded `items.tsv` |
| `digest` | C wrapper over `xxHash`, imported from Zig as `@import("c")` |
| `cli` | `manifest` binary and report formatter |
| `diagnostics` | Compiler and `zig fmt` failures |

```sh
bazel build //examples/zig-app/...
bazel test //examples/zig-app/...
bazel run //examples/zig-app/cli:manifest
bazel run //examples/zig-app/cli:manifest -- get cable
```

`manifest` prints each item, the total, the schema version and an XXH64 digest of the item lines. `get NAME` prints one item.

Exit codes: `0` ok, `2` usage, `4` unknown item. `exit_codes_test` runs the binary through each code.

Zig comes from the pinned `rules_zig` toolchain. `xxHash` is a pinned `http_archive` declared in `MODULE.bazel`. `catalog_test`, `digest_test` and `report_test` are Zig tests. `catalog_test` covers malformed input.

`diagnostics_test` compiles files with a syntax error, a type error and an unused local, and expects each to fail. It also checks every example source with `zig fmt --check`.
