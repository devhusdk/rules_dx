---
title: Gleam 1.19 needs a patched rules_gleam and a dx-owned JavaScript path
author: Martin
date: 2026-10-23
branch: issue-1791
cost: { minutes: 103, tokens: 728126 }
id: l-b6ce253f
scope: repo
status: active
touches:
  - BUILD.bazel
  - MODULE.bazel
  - acceptance/testdata/test_targets.txt
  - acceptance/v1_inventory.bzl
  - cli/adopt/src/error.rs
  - cli/adopt/src/new.rs
  - cli/cli/src/adopt/new.rs
  - cli/cli/src/args/command.rs
  - config/BUILD.bazel
  - docs/cli/commands/new-upgrade.md
---

## Decided
- `patches/rules_gleam/hermetic_gleam.patch` declares `.abstr` outputs, not `.erl` or `.cache_inline`. Gleam 1.19 `compile-package` writes those, and Bazel fails on missing declared outputs.
- Outputs go under `<target name>/` in `declare_out_file_with_ext`, so two targets in one package do not collide on the same module file.
- `warnings_as_errors` greps the compile log for `^warning`. Gleam has no flag that fails on warnings. dx wrappers in `gleam/rules/defs.bzl` default it to True, and generated Hex packages set it to False.
- The Erlang launcher template runs `erl +fnu`. Without it a UTF-8 argument such as `café` reaches `main` double-encoded.
- JavaScript is in `gleam/rules/javascript.bzl`, not in rules_gleam, which only targets Erlang. The bundle step copies package trees with `cp -RL`. Node resolves imports from real paths, so a bundle of symlinks into the execroot cannot find `../prelude.mjs`.
- `gleam_js_test` runs from the bundle directory. gleeunit's JS `main` reads `gleam.toml` and scans `test/` from the working directory.

## Watch out
- The upstream `gleam/templates/[[main_module]].sh` has no trailing newline. `gleam/toolchain/repos.bzl` drops it (DROPPED_PATHS) and the patch adds it as a new file. Regenerate the patch with `git diff` and check it has no "No newline at end of file" marker.
- With an empty suite, gleeunit prints "No tests found!" and exits 1 on both targets. `gleam/tests/negative` relies on that.
