---
title: The hermetic macOS SDK only ships frameworks listed in osx.frameworks
author: Martin
date: 2026-10-23
branch: issue-2046
cost: { minutes: 56, tokens: 422685 }
id: l-0abf7cc2
scope: repo
status: active
touches:
  - BUILD.bazel
  - MODULE.bazel
  - acceptance/testdata/test_targets.txt
  - acceptance/v1_inventory.bzl
  - config/BUILD.bazel
  - docs/cli/commands/quality.md
  - erlang/README.md
  - erlang/rules/BUILD.bazel
  - erlang/rules/defs.bzl
  - erlang/rules/wrapper_tests.bzl
---

## Decided
- `MODULE.bazel` sets `osx.frameworks` on `@llvm//extensions:osx.bzl`. The llvm module's `macos_sdk` repo extracts only the listed frameworks from Apple's CLTools SDK pkg, so a crate that links a missing one fails in ld64.lld with "framework not found".
- Any `osx.frameworks` tag replaces the module's default list (CoreFoundation, Foundation, Kernel, OSLog, Security, SystemConfiguration), so the tag repeats the defaults and adds what Rust crates need.
- `CoreServices` is needed by `fsevent-sys` (via `notify` in `//cli/cli:dx`). `CFNetwork` is needed too: `CoreServices.tbd` re-exports it, and lld errors with "unable to locate re-export" when it is absent.
## Watch out
- The macOS link can be checked from Linux: temporarily add `aarch64-apple-darwin` to `rust.toolchain(extra_target_triples)` and run `bazel build --platforms=@llvm//platforms:macos_arm64 //cli/cli:dx`. Revert before committing.
- A new framework-linking crate needs its framework (and any non-inlined re-exports in its `.tbd`) added to the list.
