---
title: Windows Rust links through lld-link from the hermetic LLVM MSVC toolchain
author: Martin
date: 2026-10-23
branch: issue-2045
cost: { minutes: 1083, tokens: 1820525 }
id: l-3b37fff4
scope: repo
status: active
touches:
  - ".bazelrc"
  - cli/env/BUILD.bazel
  - cli/env/src/lib.rs
  - quality/testdata/runner_matrix_js.bzl
  - quality/testdata/runner_matrix_python.bzl
  - quality/testdata/runner_matrix_tests.bzl
  - MODULE.bazel
  - MODULE.bazel.lock
  - apple/tests/consumer.rs
  - cargo-bazel-lock.json
---

## Decided
- `.bazelrc` sets `common:windows --host_platform=//tools/platforms:windows_msvc_host` so Rust targets get the MSVC ABI toolchain from `@llvm` (no Visual Studio). The platform carries `required_settings = ["@llvm//toolchain:runtimes_all"]`, and `@platforms//host` stays second in `--extra_execution_platforms`; without both, llvm's own stage1 tools need the MSVC toolchain that needs them, and analysis fails with a cycle.
- `patches/rules_rust/lld_link.patch` swaps the `clang-cl` link tool for its sibling `lld-link` and keeps only linker args. rustc infers link.exe-style flags from a `clang-cl` path (`/NOLOGO`, `/defaultlib`, `/PDBALTPATH`), and clang-cl rejects them.
- llvm >= 0.8.20 needs rules_cc >= 0.2.25; 0.2.25 dropped `grep-includes`, which `rules_rust_prost` 0.74.0 still references. 0.2.27 restores it.
- `blake3` gets `CARGO_FEATURE_PREFER_INTRINSICS` for `x86_64-pc-windows-msvc` via `crate.annotation_select`: its build script wants `ml64.exe` whenever `CC_x86_64_pc_windows_msvc` is unset.
## Watch out
- `startup:windows --windows_enable_symlinks` is required for `//cli/env:bootstrap_test`; without it Bazel copies `ctx.actions.symlink` outputs and the installer rejects the staged tree.
- On Windows `fs::read_link` returns `C:\...` while `fs::canonicalize` returns `\\?\C:\...`; compare canonical forms (`cli/env/src/lib.rs` `verify_installed`).
- Windows Bazel rewrites `MODULE.bazel.lock` registry URLs unless it uses the same `--registry` as `~/.bazelrc`; discard that diff.
